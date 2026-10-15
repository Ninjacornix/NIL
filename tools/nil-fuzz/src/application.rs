//! Seeded application differential oracle. Native effects are confined to fixtures.
use crate::generate::Random;
use nil_compiler::{
    SourceProfile,
    application::Host,
    compile_with_profile,
    evaluator::{Limits, Value, execute_values_with_host},
};
use nil_hir::{Diagnostic, FunctionId, Phase};
use nil_llvm::{Optimization, Options};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn fault(code: &'static str) -> Diagnostic {
    Diagnostic::new(code, Phase::Execute, None, "fixture host failure")
}
#[derive(Clone, Default)]
struct MemoryHost {
    files: BTreeMap<Vec<u8>, Vec<u8>>,
    environment: BTreeMap<Vec<u8>, Vec<u8>>,
    directories: BTreeMap<Vec<u8>, Vec<Vec<u8>>>,
    random_state: u64,
    invalid_seed: bool,
    stdout: Vec<u8>,
    writable: Vec<u8>,
    denied: bool,
    read_fault: Option<usize>,
    read_receipt: (usize, usize),
}
impl Host for MemoryHost {
    fn env(&mut self, name: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        if self.denied {
            return Err(fault("E018"));
        }
        Ok(self.environment.get(name).cloned().unwrap_or_default())
    }
    fn random(&mut self) -> Result<u64, Diagnostic> {
        if self.denied {
            return Err(fault("E018"));
        }
        if self.invalid_seed {
            return Err(fault("E016"));
        }
        Ok(nil_compiler::application::next_random(
            &mut self.random_state,
        ))
    }
    fn directory(&mut self, path: &[u8]) -> Result<Vec<Vec<u8>>, Diagnostic> {
        if self.denied {
            return Err(fault("E018"));
        }
        self.directories
            .get(path)
            .cloned()
            .ok_or_else(|| fault("E015"))
    }
    fn read(&mut self, path: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        if self.denied {
            return Err(fault("E018"));
        }
        self.files.get(path).cloned().ok_or_else(|| fault("E015"))
    }
    fn read_with_limit(&mut self, path: &[u8], limit: usize) -> Result<Vec<u8>, Diagnostic> {
        let data = self.read(path)?;
        struct Reader<'a> {
            data: &'a [u8],
            offset: usize,
            fault: Option<usize>,
            receipt: &'a mut (usize, usize),
        }
        impl std::io::Read for Reader<'_> {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                if self.fault.is_some_and(|f| self.offset >= f) {
                    self.receipt.1 += 1;
                    return Err(std::io::Error::other("injected EIO"));
                }
                let end = self.data.len().min(self.fault.unwrap_or(usize::MAX));
                let n = out.len().min(end - self.offset);
                out[..n].copy_from_slice(&self.data[self.offset..self.offset + n]);
                self.offset += n;
                self.receipt.0 += n;
                Ok(n)
            }
        }
        nil_compiler::application::read_prefix(
            Reader {
                data: &data,
                offset: 0,
                fault: self.read_fault,
                receipt: &mut self.read_receipt,
            },
            limit,
        )
    }
    fn write(&mut self, path: &[u8], data: &[u8]) -> Result<(), Diagnostic> {
        if self.denied {
            return Err(fault("E018"));
        }
        if path != self.writable {
            return Err(fault("E015"));
        }
        self.files.insert(path.to_vec(), data.to_vec());
        Ok(())
    }
    fn out(&mut self, data: &[u8]) -> Result<(), Diagnostic> {
        if self.denied {
            return Err(fault("E018"));
        }
        self.stdout.extend_from_slice(data);
        Ok(())
    }
}
fn render(value: &Value) -> Vec<u8> {
    match value {
        Value::Record(..) | Value::RecordBuffer(..) | Value::Sum(..) => {
            unreachable!("application differential entries use supported native wrappers")
        }
        Value::Map(v) => format!("{}\n", v.render()).into_bytes(),
        Value::I64(v) => format!("{v}\n").into_bytes(),
        Value::U64(_) | Value::U128(_) | Value::F64(_) => {
            format!("{}\n", nil_compiler::numeric::format(value)).into_bytes()
        }
        Value::Bool(v) => format!("{v}\n").into_bytes(),
        Value::Bytes(v) => [v.as_ref(), b"\n"].concat(),
        Value::Buffer(v) => format!(
            "[{}]\n",
            v.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
        )
        .into_bytes(),
        Value::Array(v) => format!(
            "[{}]\n",
            v.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
        )
        .into_bytes(),
    }
}
fn literal(bytes: &[u8]) -> String {
    format!(
        "\"{}\"",
        bytes
            .iter()
            .map(|v| format!("\\x{v:02X}"))
            .collect::<String>()
    )
}
// Lengths travel with expressions so random valid compositions stay well typed/bounded.
fn sequence(r: &mut Random, bytes: bool, depth: usize) -> (String, usize) {
    if depth == 0 {
        let n = r.pick(9);
        if bytes && r.pick(2) == 0 {
            let values = (0..n).map(|_| r.next_u64() as u8).collect::<Vec<_>>();
            return (literal(&values), n);
        }
        let fill = if bytes {
            r.pick(256) as i64
        } else {
            r.integer()
        };
        return (
            format!("!{}({n},{fill})", if bytes { "bytes" } else { "buffer" }),
            n,
        );
    }
    let (a, n) = sequence(r, bytes, depth - 1);
    match r.pick(4) {
        0 => {
            let (b, m) = sequence(r, bytes, depth - 1);
            (format!("!concat({a},{b})"), n + m)
        }
        1 => {
            let start = r.pick(n + 1);
            let len = r.pick(n - start + 1);
            (format!("!slice({a},{start},{len})"), len)
        }
        2 if n > 0 => {
            let index = r.pick(n);
            let fill = if bytes {
                r.pick(256) as i64
            } else {
                r.integer()
            };
            (format!("({a})[{index}:{fill}]"), n)
        }
        _ => (
            format!(
                "(true?{a}:!{}(-1,0))",
                if bytes { "bytes" } else { "buffer" }
            ),
            n,
        ),
    }
}

pub struct Case {
    pub source: String,
    pub denied: bool,
}
impl Case {
    pub fn new(seed: u64, mode: usize) -> Self {
        if mode % FAMILY_COUNT >= SET_FAMILY_START {
            return Self {
                source: crate::core_sets::cases(seed)[mode % FAMILY_COUNT - SET_FAMILY_START]
                    .1
                    .clone(),
                denied: false,
            };
        }
        if mode % FAMILY_COUNT >= SUM_FAMILY_START {
            return Self {
                source: crate::core_sums::cases(seed)[mode % FAMILY_COUNT - SUM_FAMILY_START]
                    .1
                    .clone(),
                denied: false,
            };
        }
        if mode % FAMILY_COUNT >= CHAIN_FAMILY_START {
            return Self {
                source: crate::core_chains::cases(seed)[mode % FAMILY_COUNT - CHAIN_FAMILY_START]
                    .1
                    .clone(),
                denied: false,
            };
        }
        if mode % FAMILY_COUNT >= STACK_FAMILY_START {
            let family = mode % FAMILY_COUNT - STACK_FAMILY_START;
            let depth = 200_000 + seed % 1000;
            let recursive = match family % 3 {
                0 => format!("(v,s,s,s)=#b({depth})\n1:s=a<1?\"x\":!concat(b(a-1),\"x\")"),
                1 => format!(
                    "(v,s,s,s)=#b({depth})\n1:s=a<1?\"x\":!concat(c(a-1),\"x\")\n1:s=a<1?\"x\":!concat(b(a-1),\"x\")"
                ),
                _ => {
                    let array = format!(
                        "[{}]",
                        (0..256)
                            .map(|n| (n % 256).to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                    let fields = (0..15)
                        .map(|n| format!("f{n}:256"))
                        .collect::<Vec<_>>()
                        .join(",");
                    format!(
                        "record Deep({fields})\n(v,s,s,s)=#b({depth},Deep({}))\n(i,Deep):s=a<1?\"x\":!concat(b(a-1,b),!bytes(1,b.f0[a%256]))",
                        vec![array; 15].join(",")
                    )
                }
            };
            return Self {
                source: recursive.replace("(v,s,s,s)=#b", "(v,s,s,s)=!out(\"STACK\")+#b"),
                denied: false,
            };
        }
        let mut r = Random(seed);
        let n = [0, 1, 2, 7, 31, 257][r.pick(6)];
        let v = r.integer();
        let byte = r.pick(256);
        let x = r.next_u64();
        let y = r.next_u64();
        let signature = "(v,s,s,s)";
        let expression = match mode % FAMILY_COUNT {
            n if n >= BASE_FAMILY_COUNT => crate::read_fault::case(n - BASE_FAMILY_COUNT, seed).1,
            n if n >= 475 + crate::core_operators::FAMILY_COUNT => crate::core_ownership::cases(seed)[n - 475 - crate::core_operators::FAMILY_COUNT].1.clone(),
            n if n >= 475 => crate::core_operators::cases(seed)[n - 475].1.clone(),
            0 => {
                let (s, _) = sequence(&mut r, false, 3);
                format!(":v={s}")
            }
            1 => {
                let (s, _) = sequence(&mut r, true, 3);
                format!(":s={s}")
            }
            2 => format!(":v=@(a,0;b<#a;a[b:a[b]*{v}+{}],b+1;a)", r.integer()),
            3 => format!(":s=@(b,0;b<#a;a[b:{byte}],b+1;a)"),
            4 => "=b(a[0:9])+a[0]\n(v)=a[0]".into(),
            5 => ":s=b(false?!read(\"missing\"):b)\n(s):s=!concat(a,a)".into(),
            6 => format!("=!parse(!format({v}))"),
            7 => "=!out(!read(c))+!write(d,!concat(!read(c),b))".into(),
            8 => format!("=!buffer({n},0)[{n}]"),
            9 => format!(":s=!slice(b,#b,{})", 1 + r.pick(8)),
            10 => format!(":v=!buffer(-{},0)", 1 + r.pick(1024)),
            11 => ":s=!bytes(67108864,0)".into(),
            12 => format!(":s=!bytes(1,{})", 256 + r.pick(1024)),
            13 => format!(":s=b[0:-{}]", 1 + r.pick(100)),
            14 => format!("=!parse({})", literal(&[b'0', b'1', r.pick(256) as u8])),
            15 => "=!parse(\"9223372036854775808\")".into(),
            16 => ":s=!read(!concat(c,\"/absent\"))".into(),
            17 => "=!write(!concat(d,\"/absent\"),b)".into(),
            18 => ":s=!read(\"bad\\x00path\")".into(),
            19 => "=!write(\"bad\\x00path\",b)".into(),
            20 => ":s=!read(c)".into(),
            21 => "=!write(d,b)".into(),
            22 => "=!out(b)".into(),
            23 => format!(
                ":s=@(b,0,b,0;b<{};!concat(a,\"\\xFF\"),b+1,c,d+1;!concat(a,c))",
                r.pick(8)
            ),
            24 => format!(":v=b(a,true)\n(v,b):v=b?@(a,0;b<#a;a[b:a[b]+{v}],b+1;a):!buffer(-1,0)"),
            25 => "=!out(b)+!parse(\"invalid\")".into(),
            26 => "=!write(d,b)+!read(!concat(c,\"/absent\"))[0]".into(),
            28 => ":s=!bytes(67108864,256)".into(),
            29 => ":s=!bytes(-1,256)".into(),
            30 => ":s=!slice(b,9223372036854775807,9223372036854775807)".into(),
            31 => ":s=b[#b:256]".into(),
            32 => ":v=@(a,0,a;b<#a;a[b:a[b]+1],b+1,c;!concat(a,c))".into(),
            33 => ":s=@(!bytes(0,0),0;b<0;a,b+1;a)".into(),
            34 => "=!parse(\"\")".into(),
            35 => ":s=!read(\"bad\\x00path\")".into(),
            36 => "=@(0;a<8192;a+1+#!bytes(8192,0)*0;a)".into(),
            37 => format!(":s=!format(!parse(!format({v})))"),
            38 => "=b(b[0:255],b)\n(s,s)=a[0]+b[0]".into(),
            39 => "=b(b)+b[0]\n(s)=@(a,0,0;b<#a;a[b:255],b+1,c+1;c)".into(),
            40 => "=b(a)+a[0]\n(v)=@(a,0;b<#a;a[b:a[b]+1],b+1;a)[0]".into(),
            41 => ":s=@(b,0;b<#a;b==0?a[b:255]:a,b+1;a)".into(),
            42 => ":s=b(b,b)\n(s,s):s=!concat(@(a,0;b<#a;a[b:255],b+1;a),b)".into(),
            43 => ":v=@(a,0,a;b<#a;a[b:a[b]+1],b+1,c;!concat(a,c))".into(),
            44 => ":s=@(b,0,0;b<#a;a[b:255],b+1,c+a[b];!concat(a,!format(c)))".into(),
            45 => ":s=@(b,0;b<#a;b==0?a[b:a[b]+1]:a,b+1;a)".into(),
            46 => ":s=@(b,0;b<#a;a[b:256],b+1;a)".into(),
            47 => ":s=@(b,0;b<#a;a[#a:255],b+1;a)".into(),
            48 => ":s=c(b(b))\n(s):s=!concat(a,\"x\")\n(s):s=d(a,a)\n(s,s):s=!concat(!concat(a,\"y\"),b)".into(),
            49 => ":s=@(b,0;b<3;b==0?!concat(a,\"x\"):a,b+1;a)".into(),
            50 => ":s=@(b,0,b;b<3;!concat(a,\"x\"),b+1,c;!concat(a,c))".into(),
            51 => ":s=!concat(b(b),b)\n(s):s=@(a,0;b<3;!concat(a,\"x\"),b+1;a)".into(),
            52 => ":s=@(b,0;b<5;!slice(!concat(a,\"x\"),0,#a+1)[0:255],b+1;a)".into(),
            53 => ":v=@(a,0;b<17;!concat(a,!buffer(1,b)),b+1;a)".into(),
            54 => ":s=@(b,0;b<3;!concat(a,a),b+1;a)".into(),
            55 => ":s=@(!bytes(0,0),0;b<17;!concat(!concat(a,!format(b)),\"\\n\"),b+1;a)".into(),
            56 => "=@(b,0,0;b<#a;a,b+1,c+(a[b]<128?(a[b]==10?1:2):(true?3:1/0));c)".into(),
            57 => "=@(b,0,0;b<#a?(a[b]>=0?true:false):false;a,b+1,c+a[b];c)".into(),
            58 => "=@(b,0,0;b<#a;a,b+1,c+(a[b]<256?1:1/0);c)".into(),
            59 => "=!out(\"A\")+(#b>=0?!out(b):!out(\"BAD\"))+!out(\"Z\")".into(),
            60 => "=#!concat(true?b:!bytes(-1,256),\"x\")".into(),
            61 => ":s=@(b,0,0;b<#a;a[b:255],b+1,c+(a[b]<128?(a[b]==10?1:2):3);!concat(a,!format(c)))".into(),
            62 => "=b(#b>0?b[0]:0,!bytes(17,90))\n(i,s)=a+#b".into(),
            63 => "=@(b,0,0;b<3;a,b+1,c+(true?@(0,0;a<2;a+1,b+1;b):0);c)".into(),
            64 => "=@(b,0,0;b<#a;a,b+1,c+b(a,b);c)\n(s,i)=a[b]==10?1:0".into(),
            65 => ":s=@(b,0,a;b<3;b(a),b+1,c;!concat(a,!format(#c)))\n(s):s=a".into(),
            66 => "=@(b,0,0;b<#a;a,b+1,c+(a[b]<128?b(a,b):c(a,b));c)\n(s,i)=a[b]+#a\n(s,i)=a[b]-#a".into(),
            67 => format!("=b(b,{})\n(s,i)=b>0?b(a,b-1)+a[0]:#a", 1+r.pick(23)),
            68 => format!("=b(b,{})\n(s,i)=b>0?c(a,b-1)+a[0]:#a\n(s,i)=b>0?b(a,b-1)+a[0]:#a", 1+r.pick(23)),
            69 => "=!out(\"A\")+(#b>=0?b(b):c())+!out(\"Z\")\n(s)=!out(a)\n=!out(\"BAD\")+1/0".into(),
            70 => "=b(b,7)\n(s,i)=b>0?b(!concat(a,\"x\"),b-1)+#a:#a".into(),
            71 => "=@(b,0,0;b<3;a,b+1,c+@(a,0,0;b<#a;a,b+1,c+b(a,b);c);c)\n(s,i)=a[b]<128?1:0".into(),
            72 => ":s=@(b,0;b<3;b(a,b==0),b+1;a)\n(s,b):s=b?a:!concat(a,\"x\")".into(),
            73 => "=#!concat(b(b,false),b)\n(s,b):s=b?a:!slice(a,0,#a)".into(),
            74 => "=@(b,#b-#b,0;b<#a;a,b+1,c+a[b];c)".into(),
            75 => "=@(b,#b-#b-1,0;b<#a;a,b+1,c+a[b];c)".into(),
            76 => "=@(b,#b-#b-1,0;b<#a?(b>=0?true:false):false;a,b+1,c+a[b];c)".into(),
            77 => "=@(b,0,0;b<#a;a,b+1,c+a[b];a[b])".into(),
            78 => "=@(a,0,0;b<#a;a,b+1,c+a[b+1];c)".into(),
            79 => ":s=!concat(@(b,0;b<3;b(a,3),b+1;a),b)\n(s,i):s=b>0?b(a,b-1):a".into(),
            80 => { let (a,_) = sequence(&mut r,true,2); format!(":b=!equal({a},{a})") },
            81 => { let (a,_) = sequence(&mut r,false,2); format!(":b=!equal({a},!buffer({n},{v}))") },
            82 => format!("=!find(b,{byte},{})",r.pick(2)),
            83 => format!("=!find(a,{v},0)"),
            84 => "=!find(b,256,-1)".into(),
            85 => "=!find(b,0,#b+1)".into(),
            86 => "=!find(b,256,#b)".into(),
            87 => format!(":v=!parsebuf({}, {})",literal(format!("{v};{}\n",r.integer()).as_bytes()),literal(b";\n")),
            88 => ":v=!parsebuf(\";1\",\";\")".into(),
            89 => ":v=!parsebuf(\"1;;2\",\";\")".into(),
            90 => ":v=!parsebuf(\"1;;\",\";\")".into(),
            91 => { let invalid = ["01","-0","+1","9223372036854775808","-9223372036854775809"," ","1x"]; format!(":v=!parsebuf({},\";\")",literal(invalid[r.pick(invalid.len())].as_bytes())) },
            92 => if r.pick(2)==0 {
                ":v=!parsebuf(!bytes(8388608,10),\"\\n\")".into()
            } else {
                "=b(!bytes(33554432,0))\n(s)=#!parsebuf(!bytes(4194304,10),\"\\n\")+#a".into()
            },
            93 => ":v=false?!parsebuf(\"bad\",\",\"):a".into(),
            94 => "=b(a)+#a\n(v)=#!concat(a,!parsebuf(\"3\",\",\"))".into(),
            95 => ":v=@(a,0,a;b<3;!concat(a,!parsebuf(\"3\",\",\")),b+1,c;!concat(a,c))".into(),
            96 => ":b=!equal(b[0:255],b)".into(),
            97 => "=@(b,0,0;b<#a;a,b+1,c+b(a,b);c)\n(s,i)=!find(a,a[b],b)".into(),
            98 => "=b(a,3)+#a\n(v,i)=b>0?b(!concat(a,!parsebuf(\"3\",\",\")),b-1)+#a:#a".into(),
            99 => "=!out(\"A\")+(!equal(b,b)?b():!out(\"BAD\"))+!out(\"C\")\n=!out(\"B\")".into(),
            100 => ":v=!parsebuf(\"bad\",b())\n:s=!out(\"S\")>0?\",\":\";\"".into(),
            101 => ":v=!parsebuf(\"1\\x002\\xFF-3\",\"\\x00\\xFF\\x00\")".into(),
            102 => "=b(!parsebuf(\"1,2\",\",\"))\n(v)=#!concat(a,!parsebuf(\"3\",\",\"))+a[0]".into(),
            103 => ":b=!equal(!parsebuf(\"\",\"\"),!buffer(0,9))".into(),

            104 => ":m=!map()".into(),
            105 => format!(":b=!has(!insert(!map(),{},{}),{})",literal(&[byte as u8,0]),v,literal(&[byte as u8,0])),
            106 => format!("=!get(!insert(!map(),b,{v}),b)"),
            107 => format!(":m=@(!map(),0;b<{};!insert(a,!format(b),b),b+1;a)",9+r.pick(40)),
            108 => format!("=b(!put(!map(),\"k\",{byte}))\n(m)=!get(!put(a,\"k\",7),\"k\")+!get(a,\"k\")"),
            109 => format!(":t=!insert(!bytemap(),b,!concat({},b))",literal(&[byte as u8,0])),
            110 => ":s=b(!insert(!bytemap(),\"k\",b))\n(t):s=!concat(!get(!put(a,\"k\",\"new\"),\"k\"),!get(a,\"k\"))".into(),
            111 => "=!get(!map(),b)".into(),
            112 => "=!size(!insert(!insert(!map(),b,3),b,4))".into(),
            113 => ":s=!key(!insert(!map(),b,3),1)".into(),
            114 => "=b(!insert(!map(),\"k\",3))\n(m)=!get(c(a),\"k\")+!get(a,\"k\")\n(m):m=!put(a,\"k\",9)".into(),
            115 => "=b(!put(!map(),\"k\",3))\n(m)=!get(@(a,0;b<10;!put(a,\"k\",b),b+1;a),\"k\")+!get(a,\"k\")".into(),
            116 => "=b(!put(!map(),\"k\",3))\n(m)=!get(true?!put(a,\"k\",7):a,\"k\")+!get(a,\"k\")".into(),
            117 => "=b(!put(!map(),\"k\",3))\n(m)=!get(!put(c(a),\"k\",9),\"k\")+!get(a,\"k\")\n(m):m=a".into(),
            118 => format!(":t=@(!bytemap(),0;b<{};!put(a,\"k\",!bytes(b,90)),b+1;a)",4+r.pick(15)),
            119 => format!("=!size(@(!map(),0;b<{};!put(a,!format(b),b),b+1;a))",10+r.pick(40)),
            120 => "=!size(!insert(!map(),!bytes(33554400,0),1))".into(),
            121 => "=b(!insert(!bytemap(),\"k\",!bytes(16777216,0)))\n(t)=#!get(a,\"k\")+!size(a)".into(),
            122 => "=!size(!insert(!insert(!map(),\"k\",3),\"k\",b()))\n=!out(\"V\")".into(),
            123 => "=!get(!insert(!map(),\"k\",3),\"k\")+(false?b():7)\n=!out(\"BAD\")+!get(!map(),\"missing\")".into(),
            124 => "=b(!insert(!map(),\"k\",3),7)\n(m,i)=b>0?b(a,b-1)+!get(a,\"k\"):!get(a,\"k\")".into(),
            125 => ":t=b(!insert(!bytemap(),\"k\",b))\n(t):t=!put(a,\"j\",!get(a,\"k\"))".into(),
            126 => "=!size(@(!map(),0;b<3;@(a,0;b<5;!put(a,!format(b),b),b+1;a),b+1;a))".into(),
            127 => format!(":s=!key(@(!map(),0;b<33;!insert(a,!format(b),b),b+1;a),{})",r.pick(33)),

            128 => "=b(!bytes(67108600,0))\n(s)=!size(!map())+#a".into(),
            129 => "=b(!bytes(67108600,0))\n(s)=!size(!bytemap())+#a".into(),
            130 => "=!size(!put(!map(),!bytes(33554400,0),1))".into(),
            131 => "=b(!insert(!map(),!bytes(8388608,0),1))\n(m)=c(a,!bytes(50331648,0))\n(m,s)=#!key(a,0)+#b".into(),
            132 => "=b(!insert(!bytemap(),\"k\",!bytes(4194304,0)))\n(t)=c(a,!bytes(54525952,0))\n(t,s)=#!get(a,\"k\")+#b".into(),
            133 => "=b(!insert(!map(),!bytes(8388608,0),1))\n(m)=c(a,!bytes(41943040,0))\n(m,s)=!size(!insert(a,!bytes(8388608,0),2))+#b".into(),
            134 => ":s=!sort(b,0)".into(),
            135 => ":v=!sort(a,1)".into(),
            136 => "=#!sort(!bytes(0,0),1)".into(),
            137 => format!(":v=!sort(!buffer({n},{v}),{})",r.pick(2)),
            138 => format!(":s=!sort({}, {})", literal(&(0..n).map(|_| r.next_u64() as u8).collect::<Vec<_>>()),r.pick(2)),
            139 => ":m=!sort(!put(!put(!put(!map(),\"z\",2),\"b\",1),\"a\",1),1)".into(),
            140 => ":m=!sort(!put(!put(!map(),\"z\",-9223372036854775808),\"a\",9223372036854775807),1)".into(),
            141 => ":t=!sort(!put(!put(!bytemap(),\"b\",\"\\xff\"),\"a\",\"\\0\"),1)".into(),
            142 => "=b(a)\n(v)=!sort(a,0)[0]+a[0]".into(),
            143 => ":s=b(b)\n(s):s=!concat(c(a),a)\n(s):s=!sort(a,0)".into(),
            144 => "=!get(!sort(!put(!put(!map(),\"b\",7),\"a\",8),0),\"b\")".into(),
            145 => "=!size(@(!map(),0;b<25;!sort(!put(a,!format(b),b),0),b+1;a))".into(),
            146 => "=#!sort(!bytes(40000000,0),2)".into(),
            147 => "=#!sort(!bytes(40000000,0),0)".into(),
            148 => "=!size(!sort(!insert(!insert(!map(),\"x\",1),\"x\",2),2))".into(),
            149 => "=b(a)\n(v)=!sort(true?a:!buffer(0,0),0)[0]+a[0]".into(),
            150 => "=!each(!buffer(0,0),7;1/0;a)".into(),
            151 => "=!each(\"\\xff\",0;c+b;a)".into(),
            152 => "=!each(a,0;c+b;a)".into(),
            153 => ":s=!each(!sort(!put(!put(!map(),\"z\",2),\"a\",1),1),\"\";!concat(c,!concat(a,!format(b)));a)".into(),
            154 => ":s=!each(!put(!put(!bytemap(),\"b\",\"x\"),\"a\",\"y\"),\"\";!concat(c,!concat(a,b));a)".into(),
            155 => "=!each(a,1,2;d+b,c+a;a+b)".into(),
            156 => "=b(a)\n(v)=!each(a,a,0;c[a:0],d+b;b)".into(),
            157 => "=!each(!put(!put(!map(),\"a\",1),\"b\",2),!map(),0;!put(c,a,99),d+b;b)".into(),
            158 => ":s=b(!put(!bytemap(),\"k\",\"old\"))\n(t):s=!each(a,a,\"\";!put(c,a,\"new\"),!concat(d,b);!concat(!get(a,\"k\"),b))".into(),
            159 => "=!each([1,2],0;c+b(b);a)\n1=!out(!format(a))".into(),
            160 => "=!each(a,0;c+!each([1,2],0;c+b;a)+@(0;a<2;a+1;a);a)".into(),
            161 => "=!each([1,2],0;c+(a==0?b():1);a)\n= !out(\"OK\")+(false?!out(\"BAD\")+1/0:0)".into(),
            162 => "=b(!put(!bytemap(),\"k\",!bytes(16777216,0)))\n(t)=!each(a,!bytes(16777000,0);c;#a)".into(),
            163 => "=b(!put(!map(),!bytes(8388608,0),1))\n(m)=!each(a,!bytes(48000000,0);c;#a)".into(),
            164 => ":m=!each(!buffer(8,0),!map();!sort(!put(c,!format(a),b),0);a)".into(),
            165 => ":s=!each(b,b;!concat(c,!bytes(1,b));a)".into(),
            166 => ":s=!each(!sort(!bytemap(),0),\"empty\";\"bad\";a)".into(),
            167 => "=!each([1],0;!find(\"\",256,1);a)".into(),
            168 => format!(":u64=!bits(!floatbits({x}u64)+!floatbits({y}u64))"),
            169 => format!(":u64=!bits(!floatbits({x}u64)-!floatbits({y}u64))"),
            170 => format!(":u64=!bits(!floatbits({x}u64)*!floatbits({y}u64))"),
            171 => format!(":u64=!bits(!floatbits({x}u64)/!floatbits({y}u64))"),
            172 => ":u64=!bits(!floatbits(18444509723232801281u64)+1.0)".into(),
            173 => ":u64=!bits(!parsef64(\"inf\")-!parsef64(\"inf\"))".into(),
            174 => ":u64=!bits(-0.0*1.0)".into(),
            175 => ":u64=!bits(!floatbits(1u64)*2.0)".into(),
            176 => ":u64=!bits(!floatbits(4503599627370496u64)/2.0)".into(),
            177 => ":u64=!bits(!floatbits(9218868437227405311u64)*2.0)".into(),
            178 => format!(":u64=!bits(!parsef64(!format(!floatbits({x}u64))))"),
            179 => format!(":s=!format(!floatbits({x}u64))"),
            180 => format!(":u64=!bits(!parsef64(\"{v}.125e-7\"))"),
            181 => ":u64=!bits(!parsef64(\"1.0\\x00\"))".into(),
            182 => "=!i64(!parsef64(\"nan\"))".into(),
            183 => "=!i64(9223372036854775808.0)".into(),
            184 => ":u64=!u64(-1.25)".into(),
            185 => ":u128=!u128(3.402823669209385e38)".into(),
            186 => "=!i64(-3.75)+!i64(!u128(3.75))+!i64(!u64(3.75))".into(),
            187 => format!(":u64={x}u64+{y}u64"),
            188 => format!(":u64={x}u64-{y}u64"),
            189 => format!(":u128=340282366920938463463374607431768211455u128*{x}u128"),
            190 => ":u64=1u64/0u64".into(),
            191 => ":u64=!u64(18446744073709551616u128)".into(),
            192 => ":u64=!truncu64(!trunci64(340282366920938463463374607431768211455u128))".into(),
            193 => format!(":u128=!u128({x}u64)+!u128(7)"),
            194 => format!(":u128=!parseu128(!format({x}u128*18446744073709551616u128+{y}u128))"),
            195 => ":u64=!parseu64(\"01\")".into(),
            196 => ":u128=!parseu128(\"340282366920938463463374607431768211456\")".into(),
            197 => ":u64=!bits(@(1.0,0;b<7;b(a),b+1;a))\n(f64):f64=a*1.5+0.25".into(),
            198 => ":u128=@(18446744073709551616u128,0;b<7;b(a),b+1;a)\n(u128):u128=a+1u128".into(),
            199 => ":u64=!bits(false?!f64(!i64(!parsef64(\"nan\"))):1.0)".into(),
            200 => ":u64=!bits(false?!f64(!out(\"BAD\")):1.0)".into(),
            201 => ":b=!parsef64(\"nan\")!=1.0".into(),
            202 => ":b=-0.0==0.0".into(),
            203 => format!(":b={x}u64>0u64"),
            204 => ":u64=!bits(1.0000000074505806*0.9999999925494194-1.0)".into(),
            205 => format!(":u64=!bits(!f64({x}u128*18446744073709551616u128+{y}u128))"),
            206 => ":u64=!bits(!parsef64(\"-1e-9999\"))".into(),
            207 => ":u64=!bits(!parsef64(\"1e9999\"))".into(),
            208 => format!("=R(b,a,{v}).count"),
            209 => ":s=b(R(b,a,0))\n(R):s=c(a,a.data[0:120])\n(R,s):s=!concat(a.data,b)".into(),
            210 => "=b(R(b,a,0))\n(R)=c(a,!bytes(1024,1))\n(R,s)=#a.data+#a.buffer+#b".into(),
            211 => ":s=b(b)\n(s):s=c(R(a,!buffer(0,0),0),a)\n(R,s):s=!concat(a.data,b)".into(),
            212 => "=b(Nested(R(b,a,0)))\n(Nested)=c(a,!bytes(1000,42))\n(Nested,s)=#a.inner.data+#a.inner.buffer+#b".into(),
            213 => "=b(@(R(b,a,0);a.count<7;R(!concat(a.data,\"x\"),a.buffer,a.count+1);a))\n(R)=#a.data+a.count".into(),
            214 => ":s=b(R(b,a,0))\n(R):s=(true?a{data:!concat(a.data,\"x\")}:a{data:!read(\"missing\")}).data".into(),
            215 => "=b(R(b,a,0))\n(R)=c(a,a{buffer:!concat(a.buffer,a.buffer)})\n(R,R)=#a.buffer+#b.buffer".into(),
            216 => format!(":u64=!bits(!get(!put(!map[Scalar](),\"k\",Scalar({v},!floatbits({x}u64),{y}u128,true)),\"k\").bits)"),
            217 => "=!get(!map[Pair](),\"missing\").left".into(),
            218 => "=!size(!insert(!insert(!map[Pair](),\"k\",Pair(1,2)),\"k\",Pair(3,4)))".into(),
            219 => "=!size(!sort(!map[Pair](),1))".into(),
            220 => "=!size(!sort(!map[Pair](),2))".into(),
            221 => "=b(!put(!map[Pair](),\"k\",Pair(1,2)))\n(map[Pair])=c(a,!put(a,\"k\",Pair(7,9)))\n(map[Pair],map[Pair])=!get(a,\"k\").left+!get(b,\"k\").right".into(),
            222 => "=!each(!sort(!put(!put(!map[Pair](),\"b\",Pair(3,7)),\"a\",Pair(5,9)),0),0;c+b.left+b.right;a)".into(),
            223 => "=b(@(!map[Pair](),0;b<31;!put(a,!format(b),Pair(b,b+1)),b+1;a))\n(map[Pair])=!each(a,0;c+b.left+b.right;a)".into(),
            224 => "=b(R(!bytes(40000000,0),!buffer(0,0),0))\n(R)=c(a,!bytes(30000000,1))\n(R,s)=#a.data+#b".into(),
            225 => "=b(R(!bytes(40000000,0),!buffer(0,0),0))\n(R)=#a.data+#!bytes(30000000,1)".into(),
            226 => "=(true?Pair(42,7):Pair(!out(\"BAD\"),1/0)).left".into(),
            227 => "=Pair(!out(\"A\"),!out(\"B\")).right".into(),
            228 => "=Pair(!parse(\"bad\"),1/0).left".into(),
            229 => "=b(R(b,a,3))\n(R)=a.count==0?#a.data:b(a{count:a.count-1})+a.count".into(),
            230 => "=b(R(b,a,3))\n(R)=a.count==0?#a.data:c(a{count:a.count-1})\n(R)=b(a)".into(),
            231 => "=!each(!buffer(7,2),R(b,a,0);c{count:c.count+b};a.count)".into(),
            232 => ":s=b(R(b,a,0))\n(R):s=c(a.data,a)\n(s,R):s=!concat(a,b.data)".into(),
            233 => "=!size(!each(!map[Pair](),!map[Pair]();!put(c,a,b{left:9});a))".into(),
            234 => "=b(!put(!map[Pair](),\"k\",Pair(1,2)))\n(map[Pair])=c(a,!each(a,a;!put(c,a,b{left:b.left+1});a))\n(map[Pair],map[Pair])=!get(a,\"k\").left+!get(b,\"k\").left".into(),
            235 => "=b(R(!bytes(67108824,0),a,0))\n(R)=#a.data+!size(!map[Pair]())".into(),
            236 => "=b(!put(!map[Pair](),\"\\x00\\xff\",Pair(7,9)))\n(map[Pair])=!each(!sort(a,0),0;c+b.right;a)".into(),
            237 => "=b(R(b,a,0))\n(R)=#a.data+((true?a{count:!out(\"OK\")}:a{data:!read(\"bad\")}).count)".into(),
            238 => "=b(R(b,a,0))\n(R)=c(a{data:!slice(!concat(a.data,\"tail\"),0,#a.data)},a)\n(R,R)=#a.data+#b.data".into(),
            239 => "=b(R(b,a,0))\n(R)=#!sort(a.data,0)+#a.buffer".into(),
            240 => format!("=!plugin(1,0,Packet(b,a,{v})).count"),
            241 => "=b(Packet(b,a,9))\n(Packet)=c(a,!plugin(1,0,a))\n(Packet,Packet)=a.count+b.count+#a.data".into(),
            242 => ":s=!plugin(1,0,Packet(b,a,0)).data".into(),
            243 => "=b(Packet(b,a,0))\n(Packet)=@(a;a.count<7;!plugin(1,0,a);a.count+#a.data)".into(),
            244 => "=!plugin(1,0,Packet(b,a,true?40:1/0)).count".into(),
            245 => "=(false?!plugin(1,0,Packet(!read(\"missing\"),a,1/0)):Packet(b,a,7)).count".into(),
            246 => "=!out(\"A\")+!plugin(1,0,Packet(b,a,!out(\"B\"))).count+!out(\"C\")".into(),
            247 => "=!plugin(1,0,Packet(!read(\"missing\"),a,!parse(\"bad\"))).count".into(),
            248 => "=b(Packet(!bytes(40000000,0),a,0))\n(Packet)=c(!plugin(1,0,a),!bytes(30000000,0))\n(Packet,s)=#a.data+#b".into(),
            249 => "=b(Packet(!bytes(40000000,0),a,0))\n(Packet)=#!plugin(1,0,a).data+#!bytes(30000000,0)".into(),
            250 => "=!each(a,Packet(b,a,0);!plugin(1,0,c);a.count+#a.data)".into(),
            251 => "=b(Packet(b,a,0))\n(Packet)=c(@(a,0;b<5;!plugin(1,0,a),b+1;a),a)\n(Packet,Packet)=a.count+b.count+#b.data".into(),
            252 => ":b=!equal(!plugin(1,0,Packet(b,a,0)).data,b)".into(),
            253 => ":v=!plugin(1,0,Packet(b,a,0)).buffer".into(),
            254 => "=b(Packet(b,a,7))\n(Packet)=c(a.data,!plugin(1,0,a).data)\n(s,s)=#!concat(a,b)".into(),
            255 => "=!plugin(1,0,Packet(b,a,!out(\"before\"))).count+!out(\"after\")".into(),
            256 => format!(":b=!plugin(0,0,!bytes({n},{byte}),!bytes({n},{byte}))"),
            257 => format!(":b=!equal(!buffer({n},{v}),!buffer({n},{v}))"),
            258 => ":b=!equal(\"\\xff\\0\",\"\\xff\\0\")".into(),
            259 => ":b=!equal(\"a\",\"aa\")".into(),
            260 => ":b=!equal(!bytes(0,0),!bytes(0,255))".into(),
            261 => ":b=!equal(!bytes(-1,256),!bytes(67108864,0))".into(),
            262 => "=!plugin(1,0,Packet(!bytes(67108864,0),a,!parse(\"bad\"))).count".into(),
            263 => "=!plugin(1,0,Packet(b,a,1/0)).count".into(),
            264 => format!("=!buffer[Cell]({},Cell(b,{v}))[0].value",n+1),
            265 => "=#!buffer[Cell](0,Cell(b,0))".into(),
            266 => "=b(!buffer[Cell](2,Cell(b,2)))\n(v[Cell])=c(a,a[0:Cell(\"new\",7)])\n(v[Cell],v[Cell])=a[0].value*10+b[0].value".into(),
            267 => "=b(!buffer[Cell](2,Cell(b,0)))\n(v[Cell])=c(a,!concat(a,a))\n(v[Cell],v[Cell])=#a+#b".into(),
            268 => "=#!concat(!buffer[Cell](1,Cell(b,0)),!buffer[Cell](0,Cell(b,0)))".into(),
            269 => "=b(!buffer[Row](2,Row(!buffer[Cell](1,Cell(b,3)),a,!map())))\n(v[Row])=c(a,!bytes(1024,0))\n(v[Row],s)=a[0].cells[0].value+#b".into(),
            270 => "=b(!buffer[Row](2,Row(!buffer[Cell](1,Cell(b,3)),!buffer(1,2),!map())))\n(v[Row])=c(a,a[0].data[0:9])\n(v[Row],v)=a[1].data[0]*10+b[0]".into(),
            271 => "=!each(!buffer[Cell](3,Cell(b,2)),0;c+b.value;a)".into(),
            272 => "=b(!buffer[Cell](0,Cell(b,0)))\n(v[Cell])=#!concat(a,!buffer[Cell](1,Cell(\"tail\",1)))".into(),
            273 => "=b(!buffer[Cell](0,Cell(b,0)))\n(v[Cell])=@(a,0;b<8;!concat(a,!buffer[Cell](1,Cell(\"x\",b))),b+1;a[7].value)".into(),
            274 => "=b(!buffer[Cell](1,Cell(b,2)))\n(v[Cell])=c(a,@(a,0;b<5;a[0:Cell(\"x\",b)],b+1;a))\n(v[Cell],v[Cell])=a[0].value*10+b[0].value".into(),
            275 => "=#!slice(!buffer[Cell](2,Cell(b,0)),1,1)".into(),
            276 => "=!buffer[Cell](0,Cell(b,0))[0].value".into(),
            277 => "=#!slice(!buffer[Cell](1,Cell(b,0)),2,67108864)".into(),
            278 => "=#!buffer[Cell](-1,Cell(b,0))".into(),
            279 => "=#!buffer[Cell](8388608,Cell(b,0))".into(),
            280 => "=#!sort(!buffer[Cell](0,Cell(b,0)),0)".into(),
            281 => "=#!sort(!buffer[Cell](0,Cell(b,0)),2)".into(),
            282 => "=true?!buffer[Cell](1,Cell(b,42))[0].value:!buffer[Cell](-1,Cell(b,1/0))[0].value".into(),
            283 => "=!out(\"A\")+!buffer[Cell](1,Cell(b,!out(\"B\")))[0].value+!out(\"C\")".into(),
            284 => "=b(!buffer[Cell](1,Cell(!bytes(33554432,1),0)))\n(v[Cell])=c(a,!bytes(41943040,0))\n(v[Cell],s)=a[0].text[0]+#b".into(),
            285 => "=b(!buffer[Cell](1,Cell(!bytes(33554432,1),0)))\n(v[Cell])=c(#a,!bytes(41943040,0))\n(i,s)=a+#b".into(),
            286 => "=b(!buffer[Row](1,Row(!buffer[Cell](1,Cell(b,7)),a,!map())))\n(v[Row])=#!plugin(1,0,Packet(\"kept\",!buffer(0,0),0,a)).rows[0].cells".into(),
            287 => "=!each(!buffer[Row](2,Row(!buffer[Cell](1,Cell(b,7)),!buffer(3,2),!insert(!map(),\"k\",3))),0;c+!each(b.cells,0;c+b.value;a)+#!sort(b.data,0)+!get(b.dict,\"k\");a)".into(),
            288 => "=#!buffer[Zero](3,Zero([]))[2].empty".into(),
            289 => "=#!slice(!concat(!buffer[Zero](3,Zero([]))[1:Zero([])],!buffer[Zero](2,Zero([]))),1,3)".into(),
            290 => "=#!buffer[Zero](0,Zero([]))[0].empty".into(),
            291 => "=#!buffer[Zero](8388608,Zero([]))".into(),
            292 => format!("=#!buffer[ZeroMany]({},ZeroMany([],[]))[0].right",n+1),
            293 => format!("=#!buffer[ZeroNested]({},ZeroNested(Zero([]),ZeroMany([],[])))[0].inner.empty",n+1),
            294 => format!("=!buffer[ZeroMixed](2,ZeroMixed(ZeroNested(Zero([]),ZeroMany([],[])),{v},[]))[1].value"),
            295 => "=#!get(!put(!map[Zero](),\"k\",Zero([])),\"k\").empty".into(),
            296 => "=#!get(!put(!insert(!map[ZeroMany](),\"k\",ZeroMany([],[])),\"k\",ZeroMany([],[])),\"k\").left".into(),
            297 => "=#!get(!put(!map[ZeroNested](),\"k\",ZeroNested(Zero([]),ZeroMany([],[]))),\"k\").more.right".into(),
            298 => format!("=b(!buffer[Zero](0,Zero([])))\n(v[Zero])=@(a,0;b<{};!concat(a,!buffer[Zero](1,Zero([]))),b+1;#a)",n+1),
            299 => "=b(!buffer[ZeroNested](2,ZeroNested(Zero([]),ZeroMany([],[]))))\n(v[ZeroNested])=@(a,0;b<5;a[0:ZeroNested(Zero([]),ZeroMany([],[]))],b+1;#a[0].inner.empty+#a)".into(),
            300 => "=b(!buffer[Zero](2,Zero([])))\n(v[Zero])=c(a,!concat(a,!buffer[Zero](1,Zero([]))))\n(v[Zero],v[Zero])=#a*10+#b".into(),
            301 => "=b(!map[ZeroNested]())\n(map[ZeroNested])=@(a,0;b<4;!put(a,!format(b),ZeroNested(Zero([]),ZeroMany([],[]))),b+1;!size(a))".into(),
            302 => format!("=!buffer[ZeroArray](2,ZeroArray([],[{v}]))[1].full[0]"),
            303 => "=#!get(!map[Zero](),\"missing\").empty".into(),
            304 => "=!size(!insert(!insert(!map[ZeroMany](),\"k\",ZeroMany([],[])),\"k\",ZeroMany([],[])))".into(),
            305 => "=#!buffer[ZeroNested](-1,ZeroNested(Zero([]),ZeroMany([],[])))".into(),
            306 => "=true?#!buffer[ZeroMany](2,ZeroMany([],[])):#!buffer[ZeroMany](-1,ZeroMany([],[]))".into(),
            307 => "=!each(!buffer[ZeroMixed](3,ZeroMixed(ZeroNested(Zero([]),ZeroMany([],[])),7,[])),0;c+b.value+#b.empty;a)".into(),
            308 => "=!equal(b,b)?1:0".into(),
            309 => format!("=!equal(!bytes({},{}),!bytes({},{}))?1:0",n+1,byte,n+1,byte),
            310 => format!("=!equal(!bytes({},{}),(!bytes({},{}))[{}:{}])?1:0",n+1,byte,n+1,byte,n,(byte+1)%256),
            311 => format!("=!equal(!bytes({},{}),(!bytes({},{}))[0:{}])?1:0",n+1,byte,n+1,byte,(byte+1)%256),
            312 => format!("=!equal(!bytes({n},{byte}),!bytes({},{}))?1:0",n+1,byte),
            313 => "=!equal(!bytes(0,0),!bytes(0,255))?1:0".into(),
            314 => format!("=!equal(!buffer({},{}),!buffer({},{}))?1:0",n+1,v,n+1,v),
            315 => "=!equal(!buffer(3,-9223372036854775808),!buffer(3,9223372036854775807))?1:0".into(),
            316 => "=!equal(!bytes(33554000,0),!bytes(33554000,0))?1:0".into(),
            317 => "=!equal(!bytes(40000000,0),!bytes(40000000,0))?1:0".into(),
            318 => "=(false?!equal(!bytes(-1,256),b):!equal(b,b))?1:0".into(),
            319 => "=b(b,b)\n(s,s)=@(a,b,0,0;c<7;a,b,c+1,d+(!equal(a,b)?1:0);d+#a+#b)".into(),
            320 => "=b(\"\")\n(s)=@(Cell(a,0);a.value<12;a{text:!concat(a.text,\"x\")}{value:a.value+1};#a.text)".into(),
            321 => "=b(Cell(b,0))\n(Cell)=c(a,@(a;a.value<7;a{text:!concat(a.text,\"x\")}{value:a.value+1};a))\n(Cell,Cell)=#a.text*100+#b.text".into(),
            322 => "=b(Cell(b,0))\n(Cell)=c(a{text:!concat(a.text,\"x\")},a.text)\n(Cell,s)=#a.text*100+#b".into(),
            323 => "=b(Row(!buffer[Cell](1,Cell(b,0)),a,!map()))\n(Row)=#!concat(a.cells,!buffer[Cell](1,Cell(a.cells[0].text,1)))+#a.cells".into(),
            324 => "=b(Cell(b,7))\n(Cell)=c(a{text:!concat(a.text,\"x\")},!bytes(1000,0),a.value)\n(Cell,s,i)=#a.text+#b+c".into(),
            325 => "=b(Cell(!bytes(40000000,0),7))\n(Cell)=c(a{text:!concat(a.text,\"x\")},!bytes(30000000,0),a.value)\n(Cell,s,i)=#a.text+#b+c".into(),
            326 => "=b(!buffer[Cell](0,Cell(b,0)))\n(v[Cell])=@(a,0;b<24;!concat(a,!buffer[Cell](1,Cell(\"x\",b))),b+1;!each(a,0;c+b.value+#b.text;a))".into(),
            327 => "=b(!buffer[Cell](0,Cell(b,0)),b)\n(v[Cell],s)=@(a,0,b;b<24;!concat(a,!buffer[Cell](1,Cell(c,b))),b+1,c;!each(a,0;c+b.value+#b.text;a))".into(),
            328 => "=b(!buffer[Cell](1,Cell(b,0)))\n(v[Cell])=c(a,@(a,0;b<12;!concat(a,!buffer[Cell](1,Cell(\"x\",b))),b+1;a))\n(v[Cell],v[Cell])=#a*100+#b+#a[0].text".into(),
            329 => "=b(!buffer[Cell](0,Cell(b,0)))\n(v[Cell])=@(a,0;b<3;!concat(a,!buffer[Cell](1,Cell(!bytes(30000000,0),b))),b+1;#a)".into(),
            330 => "=b(!buffer[Cell](0,Cell(b,0)))\n(v[Cell])=@(a,0;b<16;!concat(a,!buffer[Cell](1,Cell(true?\"x\":!bytes(-1,0),b))),b+1;#a)".into(),
            331 => "=b(!buffer[Cell](0,Cell(b,0)))\n(v[Cell])=@(a,0;b<16;!concat(a,!buffer[Cell](1,Cell(\"x\",b))),b+1;#!plugin(1,0,Packet(\"kept\",!buffer(0,0),0,!buffer[Row](1,Row(a,!buffer(0,0),!map())))).rows[0].cells)".into(),
            356 => ":s=!env(\"NIL_FUZZ_PRESENT\")".into(),
            357 => ":s=!env(\"NIL_FUZZ_ABSENT\")".into(),
            358 => ":s=!env(\"NIL_FUZZ_EMPTY\")".into(),
            359 => ":s=!env(\"NIL_FUZZ_RAW\")".into(),
            360 => ":s=!env(\"\")".into(),
            361 => ":s=!env(\"a=b\")".into(),
            362 => ":s=!env(\"a\\0\")".into(),
            363 => ":s=!env(\"NIL_FUZZ_PRESENT\")".into(),
            364 => "=b(!bytes(67108000,0))\n(s)=#!env(\"NIL_FUZZ_LARGE\")+#a".into(),
            365 => ":u64=!random()".into(),
            366 => ":s=!concat(!format(!random()),!format(!random()))".into(),
            367 => ":u64=false?!random():!random()".into(),
            368 => ":u64=b()\n:u64=!random()".into(),
            369 => ":u64=@(0,0u64;a<8;a+1,b+!random();b)".into(),
            370 => ":u64=!random()".into(),
            371 => ":u64=!random()".into(),
            372 => "=b(!bytes(67108000,0))\n(s)=#a+#!format(!random())".into(),
            373 => ":s=!each(!directory(!concat(c,\".entries\")),\"\";!concat(c,!concat(a,\"/\"));a)".into(),
            374 => "=!size(!directory(!concat(c,\".empty\")))".into(),
            375 => "=!size(!directory(!concat(c,\".large\")))".into(),
            376 => "=!size(!directory(!concat(c,\".missing\")))".into(),
            377 => "=!size(!directory(c))".into(),
            378 => "=!size(!directory(\"a\\0b\"))".into(),
            379 => "=!size(!directory(!concat(c,\".entries\")))".into(),
            380 => "=!size(!directory(!concat(c,\".unreadable\")))".into(),
            381 => "=b(!bytes(67108000,0),c)\n(s,s)=!size(!directory(!concat(b,\".large\")))+#a".into(),
            382 => "=b(!directory(!concat(c,\".entries\")))\n(m)=!size(a)+!size(a)".into(),
            383 => "=b(!directory(!concat(c,\".entries\")))\n(m)=@(a,0;b<4;a,b+!size(a);b)".into(),
            384 => "=false?!size(!directory(\"absent\")):7".into(),
            385 => ":s=!key(!directory(!concat(c,\".entries\")),0)".into(),
            386 => "=#!concat(b(),!env(\"NIL_FUZZ_PRESENT\"))\n:s=!env(\"NIL_FUZZ_RAW\")".into(),
            387 => ":s=@(!env(\"NIL_FUZZ_RAW\"),0;b<4;!concat(a,!env(\"NIL_FUZZ_PRESENT\")),b+1;a)".into(),
            388 => "=#!env(\"a\\0\")+!size(!directory(\"absent\"))".into(),
            389 => ":s=!env(\"=\")".into(),
            390 => "=!size(!directory(!concat(c,\".entries\")))".into(),
            391 => ":u64=!random()".into(),
            392 => format!(":v=b(&c,a)\n([i:i],v):v=@(b,0;b<#a;a[b:^a(a[b])],b+1;a)\n1=a+{v}"),
            393 => ":s=b(&c,b)\n([i:i],s):s=@(b,0;b<#a;a[b:^a(a[b])],b+1;a)\n1=a==10?32:a".into(),
            394 => "=b(&c,b)+#b\n([s:s],s)=#^a(b)\n(s):s=!concat(a,\"x\")".into(),
            395 => "=b(&c)\n([:i])=^a()\n=!out(\"selected\")".into(),
            396 => "=false?b(&c):42\n([:i])=^a()\n=!out(\"BAD\")".into(),
            397 => "=b(&c,!out(\"A\"),!out(\"B\"))+!out(\"D\")\n([i:i],i,i)=^a(b+c)\n1=!out(\"C\")+a".into(),
            398 => "=b(&c,d,b)\n([s,s:i],s,s)=^a(b,c)\n(s,s)=!write(a,b)".into(),
            399 => ":s=b(&c,c)\n([s:s],s):s=^a(b)\n(s):s=!read(a)".into(),
            400 => ":u64=b(&c)\n([:u64]):u64=^a()\n:u64=!random()".into(),
            401 => ":s=b(&c)\n([:s]):s=^a()\n:s=!env(\"NIL_FUZZ_PRESENT\")".into(),
            402 => "=b(&c)\n([:s])=#^a()\n:s=!bytes(67108865,0)".into(),
            403 => "=b(&c,b)\n([s:i],s)=^a(b)\n(s)=a[#a]".into(),
            404 => "=b(&c,!out(\"before-trap\"))\n([i:i],i)=^a(b)\n1=a/0".into(),
            405 => format!("=b(&c,{})\n([i:i],i)=b==0?0:^a(b)+b(^a,b-1)\n1=a", n % 32),
            406 => format!("=b(&c,b,{})\n([s,i:i],s,i)=^a(b,c)\n(s,i)=b==0?#a:c(a,b-1)+a[0]", n % 32),
            407 => "=b(&c,Zero([]))\n([Zero:Zero],Zero)=#!buffer[Zero](2,^a(b))\n(Zero):Zero=a".into(),
            408 => "=b(&c,ZeroNested(Zero([]),ZeroMany([],[])))\n([ZeroNested:ZeroNested],ZeroNested)=#!buffer[ZeroNested](2,^a(b))\n(ZeroNested):ZeroNested=a".into(),
            409 => ":s=b(&c,b)\n([i:i],s):s=!each(b,\"\";!concat(c,!bytes(1,^a(b)));a)\n1=a==10?32:a".into(),
            410 => format!("=b(&c,{n})\n([i:i],i)=d(^a,b)\n1=a*2\n([i:i],i)=^a(b)"),
            411 => ":b=b(&c,3,2)\n([i,i:b],i,i):b=^a(b,c)\n2:b=a<b".into(),
            412 => ":s=!plugin(3,0,&b,b)\n(s):s=!concat(a,\"local\")".into(),
            413 => ":s=b(&!plugin(3,1),b)\n([s:s],s):s=^a(b)".into(),
            414 => "=false?b(&c):#b\n([:s])=#^a()\n:s=!bytes(67108865,0)".into(),
            415 => format!("=b(&d,{})\n([i:i],i)=b==0?0:^a(b)+c(^a,b-1)\n([i:i],i)=b==0?0:^a(b)+b(^a,b-1)\n1=a", n % 32),
            416 => "=b(&c)\n([:i])=^a()\n=!out(\"denied\")".into(),
            417 => ":s=b(&c)\n([:s]):s=^a()\n:s=!env(\"NIL_FUZZ_PRESENT\")".into(),
            418 => ":s=!map(&b,b)\n1=a==10?32:a".into(),
            419 => ":v=!map(&b,a)\n1=a*2".into(),
            420 => ":s=!filter(&b,b)\n1:b=a!=10".into(),
            421 => ":v=!filter(&b,a)\n1:b=a>0".into(),
            422 => "=!fold(&b,a,0)\n2=a+b".into(),
            423 => "=!fold(&b,b,0)\n2=a+b".into(),
            424 => ":s=!map(&b,\"\")\n1=a/0".into(),
            425 => ":s=!filter(&b,\"\")\n1:b=a/0==0".into(),
            426 => ":s=false?!map(&b,b):\"safe\"\n1=!out(\"BAD\")+a/0".into(),
            427 => "=!fold(&b,\"AB\",0)\n2=a+!out(!bytes(1,b))".into(),
            428 => ":s=!map(&b,b)\n1=#!bytes(67108865,a)".into(),
            429 => ":s=!map(&b,b)\n1=256".into(),
            430 => "=!find(b,10,0)".into(),
            431 => ":b=!has(!insert(!map(),\"x\",1),\"x\")".into(),
            432 => "=!parse(\"-42\")".into(),
            433 => "=!parse(\"invalid\")".into(),
            434 => ":b=!plugin(4,0,b,b)".into(),
            435 => ":b=!plugin(4,0,b,!concat(b,\"x\"))".into(),
            436 => ":b=!plugin(4,1,a,a)".into(),
            437 => ":s=!3.0(&b,b)\n(s):s=!map(&c,a)\n1=a==10?32:a".into(),
            438 => "=b(b)\n(s)=#c(a)+a[0]\n(s):s=a[0:90]".into(),
            439 => ":s=b(b)\n(s):s=c(a,a)\n(s,s):s=!concat(a[0:90],b)".into(),
            440 => ":s=true?c(b):b\n(s):s=a\n(s):s=a[0:90]".into(),
            441 => ":s=false?c(b):b\n(s):s=a\n(s):s=a[#a:90]".into(),
            442 => ":s=b(b,3)\n(s,i):s=b==0?a:b(a[0:65+b],b-1)".into(),
            443 => ":s=b(b,3)\n(s,i):s=b==0?a:c(a[0:65+b],b-1)\n(s,i):s=b==0?a:b(a[0:65+b],b-1)".into(),
            444 => ":s=@(\"\",0;b<17;b(a,b),b+1;a)\n(s,i):s=!concat(a,!bytes(1,b))".into(),
            445 => "=b(Cell(\"ab\",0))\n(Cell)=#c(a.text)+a.text[0]\n(s):s=a[0:90]".into(),
            446 => ":s=b(\"ab\")\n(s):s=a[0:90][#a:90]".into(),
            447 => ":s=b(\"ab\")\n(s):s=!concat(a[0:90],!bytes(67108865,0))".into(),
            448 => ":s=b(\"ab\")\n(s):s=a[!out(\"before\")-6:90][!out(\"after\")-5:91]".into(),
            449 => ":s=b(\"ab\",0)\n(s,i):s=a[b-1:a[b]][b:a[b-1]]".into(),
            450 => "=b(\"ab\")\n(s)=c(a,1)[0]+a[0]\n(s,i):s=a[b-1:a[b]][b:a[b-1]]".into(),
            451 => ":s=b(\"ab\")\n(s):s=a[0:90][!out(\"middle\")-6:a[0]]".into(),
            452 => ":s=@(\"\",0;b<31;!concat(a,!bytes(1,b)),b+1;a)".into(),
            453 => ":v=@(!buffer(0,0),0;b<31;!concat(a,!buffer(1,b)),b+1;a)".into(),
            454 => ":s=@(\"\",0;b<31;!concat(a,\"x\"),b+1;a)".into(),
            455 => ":s=!concat(\"abc\",!bytes(1,256))".into(),
            456 => ":s=b(\"ab\")\n(s):s=!concat(a,!bytes(1,a[0]))".into(),
            457 => "=#!buffer[Zero](2,Zero([]))[0:Zero([])]".into(),
            458 => ":s=b(\"\")\n(s):s=a[0:0]".into(),
            459 => ":s=b(b)\n(s):s=@(a,a,0;c<#a;a,b[c:255-a[c]],c+1;b)".into(),
            460 => ":s=b(\"abc\")\n(s):s=@(a,!bytes(#a,0),0;c<#a;a,b[c:a[c+1]],c+1;b)".into(),
            461 => "=b(b,64)\n(s,i)=b==0?a[0]:c(a,!bytes(512,3),b)\n(s,s,i)=b(a,c-1)+a[0]+b[0]".into(),
            462 => "=b(!buffer[Cell](2,Cell(\"old\",7)))\n(v[Cell])=#c(a)+a[0].text[0]\n(v[Cell]):v[Cell]=@(a,0;b<50;!concat(a,!buffer[Cell](1,Cell(\"new\",b))),b+1;a)".into(),
            463 => "=b(!bytes(33554380,1))\n(s)=#c(a)+a[0]\n(s):s=a[0:2]".into(),
            464 => "=b(!bytes(33554393,1))\n(s)=#c(a)+a[0]\n(s):s=a[0:2]".into(),
            465 => "=b(b)\n(s)=#c(a)+a[0]\n(s):s=@(a,0;b<4096;!push(a,7),b+1;a)".into(),
            466 => "=b(b)\n(s)=#c(a)+a[0]\n(s):s=#!bytes(100000,3)>0?!concat(a,\"z\"):a".into(),
            467 => ":s=!push(!concat(b,!bytes(!out(\"A\")-1,0)),!out(\"B\")+64)".into(),
            468 => ":s=false?!push(b,!out(\"BAD\")+300):b".into(),
            469 => ":s=b(b)\n(s):s=!concat(!push(a,1),a)".into(),
            470 => ":s=!push(!bytes(33554393,0),1)".into(),
            471 => ":s=!push(\"\",256)".into(),
            472 => ":v=!push(!buffer(0,0),-7)".into(),
            473 => ":s=@(\"\",0;b<31;b==7?!push(a,65):!push(a,b),b+1;!push(a,90))".into(),
            474 => ":s=b(b)\n(s):s=#!push(a,1)>0?!push(a,2):a".into(),
            332..=355 => format!("=!plugin(2,{},a,b,c,d)", mode % FAMILY_COUNT - 332),
            _ => format!("=#!slice(!concat(!buffer({n},{v}),a),{n},#a)+#b"),
        };
        Self {
            source: format!(
                "{}{signature}{expression}\n",
                if mode % FAMILY_COUNT >= 264 {
                    "record Zero(empty:0)\nrecord ZeroMany(left:0,right:0)\nrecord ZeroNested(inner:Zero,more:ZeroMany)\nrecord ZeroMixed(zero:ZeroNested,value:i,empty:0)\nrecord ZeroArray(empty:0,full:1)\nrecord Cell(text:s,value:i)\nrecord Row(cells:v[Cell],data:v,dict:m)\nrecord Packet(data:s,buffer:v,count:i,rows:v[Row])\n"
                } else if mode % FAMILY_COUNT >= 240 {
                    "record Packet(data:s,buffer:v,count:i)\n"
                } else if mode % FAMILY_COUNT >= 208 {
                    "record R(data:s,buffer:v,count:i)\nrecord Nested(inner:R)\nrecord Scalar(value:i,bits:f64,wide:u128,flag:b)\nrecord Pair(left:i,right:i)\n"
                } else {
                    ""
                }
            ),
            denied: matches!(
                mode % FAMILY_COUNT,
                20..=22 | 35 | 353 | 363 | 370 | 379 | 389 | 416 | 417
            ),
        }
    }
}

/// Keeps each failed source, input, IR and binary for reproduction.
const STACK_FAMILY_START: usize = BASE_FAMILY_COUNT + crate::read_fault::FAMILY_COUNT;
const CHAIN_FAMILY_START: usize = STACK_FAMILY_START + 18;
const SUM_FAMILY_START: usize = CHAIN_FAMILY_START + crate::core_chains::FAMILY_COUNT;
const SET_FAMILY_START: usize = SUM_FAMILY_START + crate::core_sums::FAMILY_COUNT;
pub const FAMILY_COUNT: usize = SET_FAMILY_START + crate::core_sets::FAMILY_COUNT;
const BASE_FAMILY_COUNT: usize =
    475 + crate::core_operators::FAMILY_COUNT + crate::core_ownership::FAMILY_COUNT;

pub fn campaign(seed: u64, cases: usize, root: &Path, jobs: usize) -> Result<(), String> {
    if cases < FAMILY_COUNT {
        return Err(format!(
            "require at least {FAMILY_COUNT} cases for full family coverage; got {cases}"
        ));
    }
    smoke_campaign(seed, cases, root, jobs)
}
/// Explicit partial smoke suite for fast integration tests; never a full campaign.
pub fn smoke_campaign(seed: u64, cases: usize, root: &Path, jobs: usize) -> Result<(), String> {
    campaign_range(seed, cases, root, 0, jobs)
}
/// Explicit fourteen-family acquisition smoke test, not full semantic coverage.
pub fn io_fault_campaign(seed: u64, root: &Path, jobs: usize) -> Result<(), String> {
    campaign_range(
        seed,
        crate::read_fault::FAMILY_COUNT,
        root,
        BASE_FAMILY_COUNT,
        jobs,
    )
}
pub fn chain_campaign(seed: u64, root: &Path, jobs: usize) -> Result<(), String> {
    campaign_range(
        seed,
        crate::core_chains::FAMILY_COUNT,
        root,
        CHAIN_FAMILY_START,
        jobs,
    )
}
pub fn stack_campaign(seed: u64, root: &Path, jobs: usize) -> Result<(), String> {
    campaign_range(seed, 18, root, STACK_FAMILY_START, jobs)
}
/// Everything one fuzz case contributes to the campaign summary. The case
/// body only ever writes these, so cases form a pure fold and merge in any
/// order, which is what makes `--jobs` safe (Round 27 P1).
/// Held for the duration of every native run, so a case whose wall deadline
/// expires can quiesce the harness and retry before being called a timeout.
static NATIVE_RUNS: std::sync::RwLock<()> = std::sync::RwLock::new(());
/// Process-wide tally, so a test can assert the retry path was taken without
/// parsing the campaign summary. The summary keeps its own per-campaign count.
static NATIVE_RETRIES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub fn native_retries() -> usize {
    NATIVE_RETRIES.load(std::sync::atomic::Ordering::Relaxed)
}
const NATIVE_WINDOW: Duration = Duration::from_secs(10);
/// First window only. Tests shrink it to exercise the retry path; the quiesced
/// retry always gets the full window, because a retry that is also rushed would
/// prove nothing. An atomic rather than an environment variable, because
/// setting one is unsafe in this edition and the workspace forbids `unsafe`.
static FIRST_WINDOW_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(10_000);
/// Test hook: shrink the first window so the retry path is taken.
pub fn set_first_window_ms(milliseconds: u64) {
    FIRST_WINDOW_MS.store(milliseconds, std::sync::atomic::Ordering::Relaxed);
}
fn native_first_window() -> Duration {
    Duration::from_millis(FIRST_WINDOW_MS.load(std::sync::atomic::Ordering::Relaxed))
}

#[derive(Default)]
struct CaseDeltas {
    program_hashes: BTreeSet<String>,
    family_indices: BTreeMap<usize, usize>,
    inconclusive: usize,
    escalated: usize,
    escalation_families: BTreeMap<String, usize>,
    escalated_programs: BTreeSet<usize>,
    resolved: usize,
    persistent: BTreeMap<String, usize>,
    native_builds: usize,
    native_retries: usize,
    resource_codes: BTreeMap<String, usize>,
    resource_families: BTreeMap<String, usize>,
    inconclusive_programs: BTreeSet<usize>,
    native_codes: BTreeMap<String, usize>,
    codes: BTreeMap<String, usize>,
    static_codes: BTreeMap<String, usize>,
    family_codes: BTreeMap<String, BTreeMap<String, usize>>,
    operations: BTreeSet<&'static str>,
    source_mutations: BTreeMap<&'static str, usize>,
    families: BTreeMap<&'static str, usize>,
}
impl CaseDeltas {
    /// Order independent: counters add, sets union, maps add per key.
    fn merge(&mut self, other: Self) {
        self.program_hashes.extend(other.program_hashes);
        for (k, v) in other.family_indices {
            *self.family_indices.entry(k).or_default() += v;
        }
        self.inconclusive += other.inconclusive;
        self.escalated += other.escalated;
        for (k, v) in other.escalation_families {
            *self.escalation_families.entry(k).or_default() += v;
        }
        self.escalated_programs.extend(other.escalated_programs);
        self.resolved += other.resolved;
        for (k, v) in other.persistent {
            *self.persistent.entry(k).or_default() += v;
        }
        self.native_builds += other.native_builds;
        self.native_retries += other.native_retries;
        for (k, v) in other.resource_codes {
            *self.resource_codes.entry(k).or_default() += v;
        }
        for (k, v) in other.resource_families {
            *self.resource_families.entry(k).or_default() += v;
        }
        self.inconclusive_programs
            .extend(other.inconclusive_programs);
        for (k, v) in other.native_codes {
            *self.native_codes.entry(k).or_default() += v;
        }
        for (k, v) in other.codes {
            *self.codes.entry(k).or_default() += v;
        }
        for (k, v) in other.static_codes {
            *self.static_codes.entry(k).or_default() += v;
        }
        for (k, v) in other.family_codes {
            let slot = self.family_codes.entry(k).or_default();
            for (k2, v2) in v {
                *slot.entry(k2).or_default() += v2;
            }
        }
        self.operations.extend(other.operations);
        for (k, v) in other.source_mutations {
            *self.source_mutations.entry(k).or_default() += v;
        }
        for (k, v) in other.families {
            *self.families.entry(k).or_default() += v;
        }
    }
}

/// Runs one case and returns only its contribution. Pure apart from the
/// per-case directory, which is keyed by `seed + index` and so never shared.
fn case_delta(
    seed: u64,
    index: usize,
    root: &Path,
    fault_library: &Option<std::path::PathBuf>,
) -> Result<CaseDeltas, String> {
    let mut program_hashes = BTreeSet::new();
    let mut family_indices = BTreeMap::<usize, usize>::new();
    let mut inconclusive = 0usize;
    let mut escalated = 0usize;
    let mut escalation_families = BTreeMap::<String, usize>::new();
    let mut escalated_programs = BTreeSet::new();
    let mut resolved = 0usize;
    let mut persistent = BTreeMap::<String, usize>::new();
    let mut native_builds = 0usize;
    let mut native_retries = 0usize;
    let mut resource_codes = BTreeMap::<String, usize>::new();
    let mut resource_families = BTreeMap::<String, usize>::new();
    let mut inconclusive_programs = BTreeSet::new();
    let mut native_codes = BTreeMap::<String, usize>::new();
    let mut codes = BTreeMap::<String, usize>::new();
    let mut static_codes = BTreeMap::<String, usize>::new();
    let mut family_codes = BTreeMap::<String, BTreeMap<String, usize>>::new();
    let mut operations = BTreeSet::new();
    let mut source_mutations = BTreeMap::<&str, usize>::new();
    let mut families = BTreeMap::<&str, usize>::new();
    let mut family_name = match index % FAMILY_COUNT {
        10 => Some("negative_buffer_length"),
        11 => Some("bytes_allocation_guard"),
        29 => Some("negative_bytes_before_fill_range"),
        _ => None,
    };
    if let Some(name) = family_name {
        *families.entry(name).or_default() += 1;
    }
    if let Some(name) = [
        "leaf_loop",
        "sequence_return_loop",
        "calls_lazy",
        "recursion_reads",
        "mutual_recursion_reads",
        "called_effect_lazy",
        "recursive_allocation",
        "nested_call_loop",
        "conditional_sequence_alloc",
        "slice_return_alias_live",
        "computed_start",
        "negative_start",
        "negative_zero_trip",
        "exit_index",
        "shifted_index",
        "recursive_sequence_alias",
        "equal_bytes",
        "equal_buffer",
        "find_bytes",
        "find_buffer",
        "find_bounds_priority",
        "find_end_bounds",
        "find_byte_range",
        "parsebuf_fields",
        "parsebuf_leading_empty",
        "parsebuf_internal_empty",
        "parsebuf_trailing_empty",
        "parsebuf_canonical_failure",
        "parsebuf_quota_priority",
        "parsebuf_lazy",
        "parsebuf_caller_alias",
        "parsebuf_loop_alias",
        "equal_replacement_alias",
        "find_leaf_loop",
        "parsebuf_recursive_alias",
        "equal_called_effects",
        "parsebuf_argument_effect",
        "parsebuf_binary_separators",
        "parsebuf_return_alias",
        "parsebuf_empty_equal",
        "map_empty",
        "map_binary_keys",
        "map_lookup",
        "map_order_growth",
        "map_alias_old_read",
        "map_nested_byte_values",
        "map_byte_update_alias",
        "map_missing_key",
        "map_duplicate_key",
        "map_iteration_bounds",
        "map_caller_alias",
        "map_loop_alias",
        "map_lazy_alias",
        "map_returned_alias",
        "map_byte_repack",
        "map_insert_loop",
        "map_quota_boundary",
        "map_byte_result_lifetime",
        "map_duplicate_argument_effect",
        "map_lazy_called_effect",
        "map_recursive_alias",
        "map_byte_values_calls",
        "map_nested_loop",
        "map_key_iteration",
        "map_constructor_quota",
        "map_byte_constructor_quota",
        "map_put_quota",
        "map_key_result_quota",
        "map_lookup_result_quota",
        "map_duplicate_before_quota",
        "sort_bytes",
        "sort_buffer",
        "sort_empty",
        "sort_equal_elements",
        "sort_binary_bytes",
        "sort_value_key_ties",
        "sort_signed_extrema",
        "sort_byte_value_prefix",
        "sort_alias_old_read",
        "sort_caller_alias",
        "sort_rebuilt_lookup",
        "sort_loop_state",
        "sort_invalid_order_before_quota",
        "sort_quota",
        "sort_duplicate_before_order",
        "sort_lazy_alias",
        "each_empty",
        "each_single_binary",
        "each_buffer_reduce",
        "each_map_order",
        "each_owned_byte_values",
        "each_parallel_state",
        "each_snapshot_replace",
        "each_snapshot_map_update",
        "each_returned_sequence_alias",
        "each_called_effect",
        "each_nested_loop",
        "each_lazy_called_effect",
        "each_ignored_value_quota",
        "each_ignored_key_quota",
        "each_sort_update_chain",
        "each_append_snapshot_alias",
        "each_sorted_empty_map",
        "each_inherited_bounds_priority",
        "float_add_bits",
        "float_sub_bits",
        "float_mul_bits",
        "float_div_bits",
        "float_nan_payload",
        "float_inf_nan",
        "float_signed_zero",
        "float_subnormal",
        "float_min_normal",
        "float_max_finite",
        "float_roundtrip_bits",
        "float_format_exact",
        "float_decimal_parse",
        "float_parse_invalid",
        "float_to_integer_nan",
        "float_to_integer_edge",
        "float_to_unsigned_negative",
        "float_to_wide_edge",
        "float_to_integer_fraction",
        "unsigned_wrap_add",
        "unsigned_wrap_sub",
        "wide_wrap_mul",
        "unsigned_zero_division",
        "wide_checked_narrow",
        "wide_explicit_trunc",
        "mixed_width_explicit",
        "unsigned_parse_roundtrip",
        "unsigned_parse_invalid",
        "wide_parse_overflow",
        "float_called_loop",
        "wide_called_loop",
        "float_lazy_trap",
        "float_unselected_host",
        "float_comparison_nan",
        "float_zero_comparison",
        "unsigned_order",
        "float_contraction",
        "integer_float_rounding",
        "float_parse_underflow",
        "float_parse_overflow",
        "record_construct",
        "record_update_alias",
        "record_called_lifetime",
        "record_duplicate_fields",
        "record_nested_lifetime",
        "record_loop_state",
        "record_lazy_update",
        "record_buffer_alias",
        "record_map_bits",
        "record_map_missing",
        "record_map_duplicate",
        "record_map_sort_mode",
        "record_map_invalid_mode",
        "record_map_alias",
        "record_map_each_sort",
        "record_map_builder",
        "record_live_quota",
        "record_dead_quota",
        "record_lazy_trap_effect",
        "record_field_effect_order",
        "record_field_trap_order",
        "record_self_recursion",
        "record_mutual_recursion",
        "record_each_state",
        "record_returned_alias",
        "record_map_empty_each",
        "record_map_each_snapshot",
        "record_map_quota",
        "record_map_binary_key",
        "record_lazy_called_effect",
        "record_mixed_chain",
        "record_sort_field",
        "plugin_record_construct",
        "plugin_record_alias",
        "plugin_returned_sequence",
        "plugin_loop_state",
        "plugin_lazy_selected",
        "plugin_lazy_unselected",
        "plugin_effect_order",
        "plugin_argument_order",
        "plugin_live_quota",
        "plugin_dead_quota",
        "plugin_each_state",
        "plugin_loop_alias",
        "plugin_equal_returned",
        "plugin_returned_buffer",
        "plugin_call_held_alias",
        "plugin_effect_around_call",
        "plugin_equal_bytes",
        "plugin_equal_buffer",
        "plugin_equal_binary",
        "plugin_equal_length",
        "plugin_equal_empty",
        "plugin_equal_bounds_order",
        "plugin_argument_quota_order",
        "plugin_argument_division",
        "collection_construct",
        "collection_empty",
        "collection_old_alias",
        "collection_concat_alias",
        "collection_concat_empty",
        "collection_nested_lifetime",
        "collection_child_alias",
        "collection_each",
        "collection_append",
        "collection_growth_loop",
        "collection_update_loop_alias",
        "collection_slice",
        "collection_index_error",
        "collection_slice_priority",
        "collection_negative_length",
        "collection_quota",
        "collection_sort_deferred",
        "collection_sort_mode",
        "collection_lazy",
        "collection_effects",
        "collection_live_children_quota",
        "collection_dead_children_quota",
        "collection_plugin_nested",
        "collection_nested_each_sort_map",
        "collection_zero_layout",
        "collection_zero_concat_slice_update",
        "collection_zero_bounds",
        "collection_zero_quota",
        "layout_all_zero_fields",
        "layout_nested_zero_buffer",
        "layout_mixed_zero_scalar",
        "layout_zero_map",
        "layout_all_zero_map_update",
        "layout_nested_zero_map",
        "layout_zero_loop_append",
        "layout_nested_zero_loop_update",
        "layout_zero_alias",
        "layout_nested_zero_map_loop",
        "layout_mixed_zero_nonzero_array",
        "layout_zero_missing_key",
        "layout_zero_duplicate_key",
        "layout_nested_zero_negative",
        "layout_zero_lazy",
        "layout_mixed_zero_each",
        "bulk_alias_identity",
        "bulk_full_equal",
        "bulk_last_mismatch",
        "bulk_first_mismatch",
        "bulk_length_mismatch",
        "bulk_empty",
        "bulk_integer_equal",
        "bulk_integer_extremes",
        "bulk_quota_live_success",
        "bulk_quota_failure",
        "bulk_lazy_trap",
        "bulk_call_loop_alias",
        "field_append_dead_edge",
        "field_append_caller_alias",
        "field_append_old_read",
        "field_collection_old_read",
        "field_append_allocation_before_scalar_read",
        "field_append_quota_alias",
        "nested_builder_fresh",
        "nested_builder_shared",
        "nested_builder_old_alias",
        "nested_builder_quota",
        "nested_builder_lazy",
        "nested_builder_plugin_held",
        "module_multiple_functions",
        "module_transitive_import",
        "module_local_name_collision",
        "module_alias_after_return",
        "module_loop_state_allocation",
        "module_allocation",
        "module_host_effects",
        "module_lazy_unselected",
        "module_file_pipeline",
        "module_live_quota",
        "module_dead_quota",
        "module_quota_boundary",
        "module_recursive_live_sequence",
        "module_record_return",
        "module_record_buffer_children",
        "module_sort_each_plugin",
        "module_bounds_failure",
        "module_byte_range_failure",
        "module_io_failure",
        "module_parse_failure",
        "module_path_failure",
        "module_denied_host",
        "module_borrowed_calls_loop",
        "module_effect_trap_order",
        "host_env_present",
        "host_env_absent",
        "host_env_empty",
        "host_env_raw",
        "host_env_empty_name",
        "host_env_equals_name",
        "host_env_nul_name",
        "host_env_denial",
        "host_env_quota",
        "host_random_first",
        "host_random_sequence",
        "host_random_lazy",
        "host_random_callee",
        "host_random_loop",
        "host_random_denial",
        "host_random_seed_failure",
        "host_random_live_quota",
        "host_directory_order",
        "host_directory_empty",
        "host_directory_large",
        "host_directory_missing",
        "host_directory_file",
        "host_directory_nul",
        "host_directory_denial",
        "host_directory_unreadable",
        "host_directory_quota",
        "host_directory_alias",
        "host_directory_loop_state",
        "host_directory_lazy",
        "host_directory_key",
        "host_env_return_alias",
        "host_env_loop_state",
        "host_trap_order",
        "host_invalid_before_denial",
        "host_directory_entry_kinds",
        "host_random_seed_boundary",
        "hof_buffer_map",
        "hof_bytes_map",
        "hof_allocation_alias",
        "hof_selected_effect",
        "hof_unselected_effect",
        "hof_effect_order",
        "hof_write",
        "hof_read",
        "hof_random",
        "hof_env",
        "hof_quota",
        "hof_bounds",
        "hof_trap_order",
        "hof_recursion",
        "hof_recursive_sequence",
        "hof_zero_slot_record",
        "hof_nested_zero_slot_record",
        "hof_each",
        "hof_forwarding",
        "hof_comparator",
        "hof_module_template",
        "hof_module_callback",
        "hof_lazy_allocation",
        "hof_mutual_recursion",
        "hof_effect_denial",
        "hof_env_denial",
        "std_map_bytes",
        "std_map_buffer",
        "std_filter_bytes",
        "std_filter_buffer",
        "std_fold_buffer",
        "std_fold_bytes",
        "std_empty_map",
        "std_empty_filter",
        "std_lazy_unselected",
        "std_effect_order",
        "std_callback_quota",
        "std_byte_range",
        "find_retained_core",
        "has_retained_core",
        "parse_retained_core",
        "parse_failure",
        "mirrored_equal_alias",
        "mirrored_equal_length",
        "mirrored_equal_buffer",
        "std_qualified_module",
        "transfer_live_alias",
        "transfer_return_alias",
        "transfer_lazy_selected",
        "transfer_lazy_unselected",
        "transfer_recursion",
        "transfer_mutual_recursion",
        "transfer_loop_builder",
        "transfer_record_child_alias",
        "transfer_bounds_partial",
        "transfer_quota_partial",
        "transfer_effect_order",
        "transfer_swap_bounds",
        "transfer_swap_alias",
        "transfer_swap_effect_barrier",
        "singleton_bytes_builder",
        "singleton_buffer_builder",
        "singleton_literal_builder",
        "singleton_byte_range",
        "singleton_alias_rhs",
        "transfer_zero_slot_records",
        "transfer_empty_bounds",
        "retained_read_alias",
        "retained_read_shifted",
        "reconcile_deep_pending_calls",
        "reconcile_child_growth",
        "reconcile_quota_success",
        "reconcile_quota_failure",
        "reconcile_cross_frame_relocation",
        "reconcile_callee_collection",
        "inline_push_argument_effects",
        "inline_push_unselected_effect",
        "inline_push_live_alias",
        "inline_push_quota",
        "inline_push_byte_range",
        "inline_push_empty_buffer",
        "inline_push_nested_regions",
        "inline_push_capture_after_expansion",
    ]
    .get((index % FAMILY_COUNT).wrapping_sub(64))
    {
        *families.entry(name).or_default() += 1;
        family_name = Some(*name);
    }
    let case_seed = seed.wrapping_add(index as u64);
    if (475..475 + crate::core_operators::FAMILY_COUNT).contains(&(index % FAMILY_COUNT)) {
        let name = crate::core_operators::cases(case_seed)[index % FAMILY_COUNT - 475].0;
        *families.entry(name).or_default() += 1;
        family_name = Some(name);
    }
    if (475 + crate::core_operators::FAMILY_COUNT..BASE_FAMILY_COUNT)
        .contains(&(index % FAMILY_COUNT))
    {
        let name = crate::core_ownership::cases(case_seed)
            [index % FAMILY_COUNT - 475 - crate::core_operators::FAMILY_COUNT]
            .0;
        *families.entry(name).or_default() += 1;
        family_name = Some(name);
    }
    let acquisition = (BASE_FAMILY_COUNT..STACK_FAMILY_START)
        .contains(&(index % FAMILY_COUNT))
        .then(|| crate::read_fault::case(index % FAMILY_COUNT - BASE_FAMILY_COUNT, case_seed));
    if let Some((name, _, _)) = &acquisition {
        *families.entry(name).or_default() += 1;
        family_name = Some(*name);
    }
    if index % FAMILY_COUNT >= SET_FAMILY_START {
        let name = crate::core_sets::cases(case_seed)[index % FAMILY_COUNT - SET_FAMILY_START].0;
        *families.entry(name).or_default() += 1;
        family_name = Some(name);
    } else if index % FAMILY_COUNT >= SUM_FAMILY_START {
        let name = crate::core_sums::cases(case_seed)[index % FAMILY_COUNT - SUM_FAMILY_START].0;
        *families.entry(name).or_default() += 1;
        family_name = Some(name);
    } else if index % FAMILY_COUNT >= CHAIN_FAMILY_START {
        let name =
            crate::core_chains::cases(case_seed)[index % FAMILY_COUNT - CHAIN_FAMILY_START].0;
        *families.entry(name).or_default() += 1;
        family_name = Some(name);
    }
    let stack_family = (STACK_FAMILY_START..CHAIN_FAMILY_START)
        .contains(&(index % FAMILY_COUNT))
        .then(|| index % FAMILY_COUNT - STACK_FAMILY_START);
    if let Some(family) = stack_family {
        let name = [
            "unbounded_stack_sequence",
            "unbounded_stack_mutual",
            "unbounded_stack_large_record",
        ][family % 3];
        *families.entry(name).or_default() += 1;
        family_name = Some(name);
    }
    let case = Case::new(case_seed, index);
    if index % FAMILY_COUNT >= 475 {
        for (name, source, code) in [
            ("operator_mixed_width", "=1u64&1", "E007"),
            ("operator_unsigned_shift_amount", "=1u128<<1u64", "E007"),
            ("operator_float_remainder", "=1.0%2.0", "E007"),
            ("logical_wrong_type", "=true&&1", "E007"),
            ("binding_unknown", "=value=1;missing", "E005"),
            ("binding_reserved", "=nan=1;nan", "E001"),
            ("binding_missing_separator", "=value=1 value", "E001"),
            ("literal_bad_radix", "=0xg", "E001"),
        ] {
            let a = compile_case(source).unwrap_err();
            let b = compile_case(source).unwrap_err();
            if a != b || a.code != code {
                return Err(format!("operator mutation {name}: {a:?}"));
            }
            *static_codes.entry(a.code.into()).or_default() += 1;
            *source_mutations.entry(name).or_default() += 1;
        }
    }
    *family_indices.entry(index % FAMILY_COUNT).or_default() += 1;
    let hash = case.source.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    });
    program_hashes.insert(format!("{hash:016x}"));
    if index % FAMILY_COUNT >= 392 {
        for (name, source, code) in [
            ("hof_unknown_reference", "=b(&z,1)\n([i:i],i)=^a(b)", "E004"),
            (
                "hof_wrong_signature",
                "=b(&c,1)\n([i:i],i)=^a(b)\n(b)=1",
                "E007",
            ),
            ("hof_malformed_type", "([i:])=1", "E002"),
            ("hof_unknown_parameter", "=^a(1)", "E005"),
            ("hof_function_escape", "=&a", "E007"),
        ] {
            let a = compile_case(source).unwrap_err();
            let b = compile_case(source).unwrap_err();
            if a != b || a.code != code {
                return Err(format!("HOF mutation {name}: {a:?}"));
            }
            *static_codes.entry(a.code.into()).or_default() += 1;
            *source_mutations.entry(name).or_default() += 1;
        }
    }
    if index % FAMILY_COUNT >= 264 {
        for (name, source) in collection_mutations(&case.source) {
            if source == case.source {
                continue;
            }
            let a = compile_case(&source);
            let b = compile_case(&source);
            match (a, b) {
                (Ok(a), Ok(b)) => assert_eq!(a.hir, b.hir),
                (Err(a), Err(b)) => assert_eq!(a, b),
                _ => return Err("nondeterministic collection mutation".into()),
            }
            *source_mutations.entry(name).or_default() += 1;
        }
    }
    if index % FAMILY_COUNT >= 240 {
        for (name, source) in plugin_mutations(&case.source) {
            if source == case.source {
                continue;
            }
            let a = compile_case(&source);
            let b = compile_case(&source);
            match (a, b) {
                (Ok(a), Ok(b)) => assert_eq!(a.hir, b.hir),
                (Err(a), Err(b)) => assert_eq!(a, b),
                _ => return Err("nondeterministic plugin mutation".into()),
            }
            *source_mutations.entry(name).or_default() += 1;
        }
    }
    if (168..208).contains(&(index % FAMILY_COUNT)) {
        for (name, mutated) in numeric_mutations(&case.source) {
            if mutated == case.source {
                continue;
            }
            crate::frontend_with_profile(&mutated, SourceProfile::ExprV5);
            *source_mutations.entry(name).or_default() += 1;
        }
    }
    if (208..240).contains(&(index % FAMILY_COUNT)) {
        for (name, mutated) in record_mutations(&case.source) {
            if mutated == case.source {
                continue;
            }
            crate::frontend_with_profile(&mutated, SourceProfile::ExprV5);
            *source_mutations.entry(name).or_default() += 1;
        }
    }
    if case.source.contains("!each(") {
        for (name, mutated) in each_mutations(&case.source) {
            crate::frontend_with_profile(&mutated, SourceProfile::ExprV5);
            *source_mutations.entry(name).or_default() += 1;
        }
    }
    let folder = root.join(format!("v5-{case_seed}"));
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    fs::write(folder.join("source.nil"), &case.source).map_err(|e| e.to_string())?;
    fs::write(
        folder.join("replay.txt"),
        format!("seed={seed} index={index} denied={}\n", case.denied),
    )
    .map_err(|e| e.to_string())?;
    let compiled = compile_case(&case.source).map_err(|e| format!("{}: {e}", folder.display()))?;
    fs::write(folder.join("module.ll"), nil_llvm::emit_llvm(&compiled.hir))
        .map_err(|e| e.to_string())?;
    // These count generated syntax coverage; errors below count executed outcomes.
    for op in [
        "env",
        "random",
        "directory",
        "plugin",
        "buffer",
        "bytes",
        "concat",
        "slice",
        "format",
        "parse",
        "read",
        "write",
        "out",
        "equal",
        "find",
        "parsebuf",
        "map",
        "bytemap",
        "has",
        "size",
        "key",
        "get",
        "put",
        "insert",
        "sort",
        "each",
        "i64",
        "u64",
        "u128",
        "f64",
        "trunci64",
        "truncu64",
        "bits",
        "floatbits",
        "parseu64",
        "parseu128",
        "parsef64",
    ] {
        if case.source.contains(&format!("!{op}(")) {
            operations.insert(op);
        }
    }
    let input = folder.join(if acquisition.is_some() {
        "nil-quota-fault.bin"
    } else {
        "input"
    });
    let output = folder.join("output");
    let mut r = Random(case_seed ^ 0xa11a5);
    let buffer = (0..1 + r.pick(16)).map(|_| r.integer()).collect::<Vec<_>>();
    let text = (0..1 + r.pick(16))
        .map(|_| b' ' + r.pick(95) as u8)
        .collect::<Vec<_>>();
    let file = (0..if index % FAMILY_COUNT == BASE_FAMILY_COUNT + 12 {
        64
    } else if acquisition.is_some() {
        8192
    } else {
        r.pick(10000)
    })
        .map(|_| r.next_u64() as u8)
        .collect::<Vec<_>>();
    let input_bytes = input.to_str().unwrap().as_bytes().to_vec();
    let output_bytes = output.to_str().unwrap().as_bytes().to_vec();
    let args = vec![
        Value::Buffer(buffer.clone().into()),
        Value::Bytes(text.clone().into()),
        Value::Bytes(input_bytes.clone().into()),
        Value::Bytes(output_bytes.clone().into()),
    ];
    let native_args = vec![
        format!(
            "[{}]",
            buffer
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ),
        String::from_utf8(text.clone()).unwrap(),
        input.to_str().unwrap().into(),
        output.to_str().unwrap().into(),
    ];
    fs::write(folder.join("arguments.txt"), format!("{native_args:?}\n"))
        .map_err(|e| e.to_string())?;
    let environment = BTreeMap::from([
        (b"NIL_FUZZ_PRESENT".to_vec(), text.clone()),
        (b"NIL_FUZZ_EMPTY".to_vec(), vec![]),
        (b"NIL_FUZZ_RAW".to_vec(), vec![255, 128, b'x']),
        (b"NIL_FUZZ_LARGE".to_vec(), vec![b'x'; 1024]),
    ]);
    let mut directories = BTreeMap::new();
    for (suffix, count) in [
        (".entries", 16),
        (".empty", 0),
        (".large", 1024),
        (".unreadable", 0),
    ] {
        if index % FAMILY_COUNT < 373 {
            continue;
        }
        let path = std::path::PathBuf::from(format!("{}{suffix}", input.display()));
        fs::create_dir(&path).map_err(|e| e.to_string())?;
        let mut names = vec![];
        for i in (0..count).rev() {
            let name = format!("entry-{i:04}-é").into_bytes();
            fs::write(path.join(std::str::from_utf8(&name).unwrap()), b"")
                .map_err(|e| e.to_string())?;
            names.push(name);
        }
        if suffix == ".entries" {
            fs::create_dir(path.join("subdirectory")).map_err(|e| e.to_string())?;
            names.push(b"subdirectory".to_vec());
        }
        if suffix == ".unreadable" && index % FAMILY_COUNT == 380 {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o000))
                    .map_err(|e| e.to_string())?;
            }
        } else {
            directories.insert(path.to_str().unwrap().as_bytes().to_vec(), names);
        }
    }
    let mut host = MemoryHost {
        environment: environment.clone(),
        directories,
        random_state: case_seed,
        invalid_seed: index % FAMILY_COUNT == 371,
        files: BTreeMap::from([
            (input_bytes, file.clone()),
            (output_bytes.clone(), b"sentinel".to_vec()),
        ]),
        writable: output_bytes.clone(),
        denied: case.denied,
        read_fault: acquisition.as_ref().and_then(|(_, _, fault)| *fault),
        ..Default::default()
    };
    let initial_host = host.clone();
    let limits = Limits {
        steps: 1_000_000,
        call_depth: stack_family.map_or(256, |family| if family < 9 { 32 } else { 1024 }),
        memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
        defer_slot_writes: false,
    };
    let expected = execute_values_with_host(&compiled.hir, FunctionId(0), &args, limits, &mut host);
    let code = expected.as_ref().err().map(|e| e.code).unwrap_or("OK");
    *codes.entry(code.into()).or_default() += 1;
    if let Some(name) = family_name {
        *family_codes
            .entry(name.into())
            .or_default()
            .entry(code.into())
            .or_default() += 1;
    }
    let original_reference_ok = expected.is_ok();
    for opt in [Optimization::O0, Optimization::O2] {
        let mut host = host.clone();
        let mut expected = expected.clone();
        for replay in 0..=1 {
            let scale = if replay == 0 { 1 } else { 4 };
            if replay == 1 {
                host = initial_host.clone();
                expected = execute_values_with_host(
                    &compiled.hir,
                    FunctionId(0),
                    &args,
                    Limits {
                        steps: limits.steps * 4,
                        call_depth: limits.call_depth * 4,
                        memory_bytes: limits.memory_bytes * 4,
                        defer_slot_writes: false,
                    },
                    &mut host,
                );
            }
            let code = expected.as_ref().err().map(|e| e.code).unwrap_or("OK");
            let expected_result = expected.as_ref().map(render).unwrap_or_default();
            fs::write(&input, &file).map_err(|e| e.to_string())?;
            fs::write(&output, b"sentinel").map_err(|e| e.to_string())?;
            let binary = folder.join(format!("native-{}-x{scale}", opt.flag()));
            let result_path = folder.join(format!("result-{}-x{scale}.bin", opt.flag()));
            nil_llvm::build_with_result_receipt(
                &compiled.hir,
                &binary,
                Options {
                    optimization: opt,
                    memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64 * scale,
                    steps: limits.steps * scale,
                    call_depth: limits.call_depth as u64 * scale,
                    stack_bytes: stack_family.map_or(Options::default().stack_bytes, |family| {
                        [64 * 1024, 1024 * 1024, 8 * 1024 * 1024][family / 3 % 3]
                    }) * scale,
                    ..Default::default()
                },
                &result_path,
            )
            .map_err(|e| e.to_string())?;
            native_builds += 1;
            let mut command = Command::new(binary);
            command
                .args(&native_args)
                .env_remove("NIL_DENY_HOST_IO")
                .env_remove("NIL_FUZZ_ABSENT");
            command.env(
                "NIL_RANDOM_SEED",
                if index % FAMILY_COUNT == 371 {
                    "01".into()
                } else {
                    case_seed.to_string()
                },
            );
            for (name, value) in &environment {
                #[cfg(unix)]
                {
                    use std::os::unix::ffi::OsStringExt;
                    command.env(
                        std::ffi::OsString::from_vec(name.clone()),
                        std::ffi::OsString::from_vec(value.clone()),
                    );
                }
            }
            if case.denied {
                command.env("NIL_DENY_HOST_IO", "1");
            }
            if let Some((_, _, Some(offset))) = &acquisition {
                command.env(
                    if cfg!(target_os = "macos") {
                        "DYLD_INSERT_LIBRARIES"
                    } else {
                        "LD_PRELOAD"
                    },
                    fault_library.as_ref().unwrap(),
                );
                command.env("NIL_AUDIT_FAIL_AFTER", offset.to_string());
                command.env(
                    "NIL_AUDIT_TRACE",
                    folder.join(format!("read-fault-{}-x{scale}.json", opt.flag())),
                );
            }
            let stdout_path = folder.join("stdout.bin");
            let stderr_path = folder.join("stderr.txt");
            command.stdout(Stdio::from(
                fs::File::create(&stdout_path).map_err(|e| e.to_string())?,
            ));
            command.stderr(Stdio::from(
                fs::File::create(&stderr_path).map_err(|e| e.to_string())?,
            ));
            let mut child = command.spawn().map_err(|e| e.to_string())?;
            // The deadline is wall clock, so under load it used to fail cases
            // that were merely slow: a concurrent gate run reported
            // "case 147: native timeout" for a program that is not slow at all.
            // A first expiry is therefore treated as suspect rather than fatal.
            // The harness is quiesced — every other native run holds a read
            // guard for its duration, so taking the write guard waits them out
            // — and the same child is given one more window. Only a second
            // expiry, with nothing else running, is a real timeout.
            let mut reading = Some(NATIVE_RUNS.read().expect("native lock"));
            let mut writing = None;
            let mut deadline = Instant::now() + native_first_window();
            let status = loop {
                if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                    break status;
                }
                if Instant::now() >= deadline {
                    if writing.is_none() {
                        native_retries += 1;
                        NATIVE_RETRIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        // Release before acquiring, or this thread deadlocks
                        // against its own read guard.
                        reading = None;
                        writing = Some(NATIVE_RUNS.write().expect("native lock"));
                        deadline = Instant::now() + NATIVE_WINDOW;
                        continue;
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "native timeout after a quiesced retry: {}",
                        folder.display()
                    ));
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            drop(reading);
            drop(writing);
            let actual = std::process::Output {
                status,
                stdout: fs::read(stdout_path).map_err(|e| e.to_string())?,
                stderr: fs::read(stderr_path).map_err(|e| e.to_string())?,
            };
            let actual_code =
                native_outcome(&actual).map_err(|e| format!("seed={case_seed}: {e}"))?;
            *native_codes
                .entry(format!("{}:x{scale}:{actual_code}", opt.flag()))
                .or_default() += 1;
            let resource =
                matches!(code, "E008" | "E013") || matches!(actual_code, "E008" | "E013");
            if let Some((_, _, Some(_))) = &acquisition {
                let trace = fs::read_to_string(
                    folder.join(format!("read-fault-{}-x{scale}.json", opt.flag())),
                )
                .map_err(|e| format!("missing native fault receipt: {e}"))?;
                let count = |key: &str| -> Result<usize, String> {
                    trace
                        .split_once(&format!("\"{key}\":"))
                        .and_then(|(_, tail)| tail.split([',', '}']).next())
                        .and_then(|v| v.parse().ok())
                        .ok_or_else(|| format!("invalid fault receipt: {trace}"))
                };
                let receipt = (count("prefix_bytes")?, count("EIO_returns")?);
                if !resource && receipt != host.read_receipt {
                    return Err(format!(
                        "fault acquisition divergence seed={case_seed}: native={receipt:?} reference={:?}",
                        host.read_receipt
                    ));
                }
            }
            fs::write(folder.join("stdout.bin"), &actual.stdout).map_err(|e| e.to_string())?;
            fs::write(folder.join("stderr.txt"), &actual.stderr).map_err(|e| e.to_string())?;
            fs::write(
                folder.join(format!("stdout-{}-x{scale}.bin", opt.flag())),
                &actual.stdout,
            )
            .map_err(|e| e.to_string())?;
            fs::write(
                folder.join(format!("stderr-{}-x{scale}.txt", opt.flag())),
                &actual.stderr,
            )
            .map_err(|e| e.to_string())?;
            if fs::read(&input).map_err(|e| e.to_string())? != file
                || actual.stdout != host.stdout
                || fs::read(&output).map_err(|e| e.to_string())? != host.files[&output_bytes]
            {
                return Err(format!(
                    "host effect/input divergence seed={case_seed} mode={} {} x{scale}; {}",
                    index % FAMILY_COUNT,
                    opt.flag(),
                    folder.display()
                ));
            }
            if resource {
                let receipt = format!("reference-{}-x{scale}", opt.flag());
                fs::write(
                    folder.join(format!("{receipt}.txt")),
                    match &expected {
                        Ok(_) => "OK".to_owned(),
                        Err(error) => error.to_string(),
                    },
                )
                .map_err(|e| e.to_string())?;
                fs::write(folder.join(format!("{receipt}-stdout.bin")), &host.stdout)
                    .map_err(|e| e.to_string())?;
                fs::write(
                    folder.join(format!("{receipt}-output.bin")),
                    &host.files[&output_bytes],
                )
                .map_err(|e| e.to_string())?;
                if replay == 0 {
                    *escalation_families
                        .entry(format!(
                            "{}:{}:reference={code}:native={actual_code}",
                            family_name.unwrap_or("unnamed"),
                            resource_direction(code, actual_code)
                        ))
                        .or_default() += 1;
                    escalated += 1;
                    escalated_programs.insert(index);
                    continue;
                }
                if persistent_resource_direction(original_reference_ok, code, actual_code).is_err()
                {
                    return Err(format!(
                        "rule-3 native E013 after 4x escalation with original reference success: seed={case_seed} family={} {}",
                        family_name.unwrap_or("unnamed"),
                        folder.display()
                    ));
                }
                let direction =
                    persistent_resource_direction(original_reference_ok, code, actual_code)?;
                *persistent
                    .entry(format!(
                        "{}:{direction}:reference={code}:native={actual_code}",
                        family_name.unwrap_or("unnamed")
                    ))
                    .or_default() += 1;
                fs::write(
                    folder.join("reference.txt"),
                    match &expected {
                        Ok(_) => "OK".to_owned(),
                        Err(e) => e.to_string(),
                    },
                )
                .map_err(|e| e.to_string())?;
                fs::write(folder.join("reference-stdout.bin"), &host.stdout)
                    .map_err(|e| e.to_string())?;
                fs::write(
                    folder.join("reference-output.bin"),
                    &host.files[&output_bytes],
                )
                .map_err(|e| e.to_string())?;
                let resource_code = if matches!(actual_code, "E008" | "E013") {
                    actual_code
                } else {
                    code
                };
                *resource_codes.entry(resource_code.into()).or_default() += 1;
                *resource_families
                    .entry(format!(
                        "{}:{resource_code}",
                        family_name.unwrap_or("unnamed")
                    ))
                    .or_default() += 1;
                inconclusive += 1;
                inconclusive_programs.insert(index);
                break;
            }
            if actual_code != code
                || fs::read(&result_path).unwrap_or_default() != expected_result
                || fs::read(&output).map_err(|e| e.to_string())? != host.files[&output_bytes]
                || fs::read(&input).map_err(|e| e.to_string())? != file
            {
                return Err(format!(
                    "divergence seed={case_seed} mode={} {} expected={code} native={actual_code}; {}",
                    index % FAMILY_COUNT,
                    opt.flag(),
                    folder.display()
                ));
            }
            if let Some(span) = expected.as_ref().err().and_then(|error| error.span) {
                let prefix = format!("{code} @{}..{} ", span.start, span.end);
                if !actual.stderr.starts_with(prefix.as_bytes()) {
                    return Err(format!(
                        "diagnostic-span divergence seed={case_seed} mode={} {} expected={prefix}; {}",
                        index % FAMILY_COUNT,
                        opt.flag(),
                        folder.display()
                    ));
                }
            }
            if replay == 1 {
                resolved += 1;
            }
            break;
        }
    }
    #[cfg(unix)]
    if index % FAMILY_COUNT == 380 {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            format!("{}.unreadable", input.display()),
            fs::Permissions::from_mode(0o700),
        )
        .map_err(|e| e.to_string())?;
    }
    if !escalated_programs.contains(&index) {
        fs::remove_dir_all(folder).map_err(|e| e.to_string())?;
    }
    Ok(CaseDeltas {
        program_hashes,
        family_indices,
        inconclusive,
        escalated,
        escalation_families,
        escalated_programs,
        resolved,
        persistent,
        native_builds,
        native_retries,
        resource_codes,
        resource_families,
        inconclusive_programs,
        native_codes,
        codes,
        static_codes,
        family_codes,
        operations,
        source_mutations,
        families,
    })
}

fn campaign_range(
    seed: u64,
    cases: usize,
    root: &Path,
    first: usize,
    jobs: usize,
) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut totals = CaseDeltas::default();
    let fault_library = if first + cases > BASE_FAMILY_COUNT {
        Some(crate::read_fault::build_library(&root)?)
    } else {
        None
    };
    module_manifest_mutations(
        seed,
        &root,
        &mut totals.source_mutations,
        &mut totals.families,
    )?;
    // Cases are a pure fold, so they may run in any order and on any thread;
    // the merge is order independent and `--jobs 1` is the sequential path.
    if jobs <= 1 {
        for index in first..first + cases {
            totals.merge(case_delta(seed, index, &root, &fault_library)?);
        }
    } else {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let next = AtomicUsize::new(first);
        let end = first + cases;
        let outcomes = std::thread::scope(|scope| {
            let workers = (0..jobs)
                .map(|_| {
                    scope.spawn(|| {
                        let mut local = CaseDeltas::default();
                        let mut failure: Option<(usize, String)> = None;
                        loop {
                            let index = next.fetch_add(1, Ordering::Relaxed);
                            if index >= end {
                                break;
                            }
                            match case_delta(seed, index, &root, &fault_library) {
                                Ok(delta) => local.merge(delta),
                                Err(error) => {
                                    // Keep this worker's lowest failure; the
                                    // campaign reports the lowest overall.
                                    if failure.as_ref().is_none_or(|(at, _)| index < *at) {
                                        failure = Some((index, error));
                                    }
                                }
                            }
                        }
                        (local, failure)
                    })
                })
                .collect::<Vec<_>>();
            workers
                .into_iter()
                .map(|worker| worker.join().expect("fuzz worker panicked"))
                .collect::<Vec<_>>()
        });
        let mut lowest: Option<(usize, String)> = None;
        for (local, failure) in outcomes {
            totals.merge(local);
            if let Some((index, error)) = failure {
                if lowest.as_ref().is_none_or(|(at, _)| index < *at) {
                    lowest = Some((index, error));
                }
            }
        }
        if let Some((index, error)) = lowest {
            return Err(format!("case {index}: {error}"));
        }
    }
    let CaseDeltas {
        program_hashes,
        family_indices,
        inconclusive,
        escalated,
        escalation_families,
        escalated_programs,
        resolved,
        persistent,
        native_builds,
        native_retries,
        resource_codes,
        resource_families,
        inconclusive_programs,
        native_codes,
        codes,
        static_codes,
        family_codes,
        operations,
        source_mutations,
        families,
    } = totals;
    let inconclusive_programs = inconclusive_programs.into_iter().collect::<Vec<_>>();
    let operations = operations.into_iter().collect::<Vec<_>>();
    let family_indices = (0..FAMILY_COUNT)
        .map(|i| family_indices.get(&i).copied().unwrap_or(0))
        .collect::<Vec<_>>();
    let program_hashes = program_hashes.into_iter().collect::<Vec<_>>();
    println!(
        "{{\"seed\":{seed},\"programs\":{cases},\"native_builds\":{native_builds},\"native_retries\":{native_retries},\"escalated_comparisons\":{escalated},\"escalation_families\":{escalation_families:?},\"escalated_programs\":{},\"resolved_and_matched\":{resolved},\"persistent_resources\":{persistent:?},\"rule_3_failures\":0,\"divergences\":0,\"non_resource_divergences\":0,\"resource_codes\":{resource_codes:?},\"resource_families\":{resource_families:?},\"inconclusive_comparisons\":{inconclusive},\"inconclusive_program_indices\":{inconclusive_programs:?},\"native_codes\":{native_codes:?},\"observed_codes\":{codes:?},\"generated_intrinsics\":{operations:?},\"call_families\":{families:?},\"source_mutations\":{source_mutations:?},\"static_codes\":{static_codes:?},\"family_codes\":{family_codes:?},\"family_indices\":{family_indices:?},\"program_hashes\":{program_hashes:?}}}",
        escalated_programs.len()
    );
    Ok(())
}

/// Malformed and mutation-accepted structured regions must have deterministic
/// checking, valid spans, revalidated HIR and bounded execution; never infer
/// source validity from a mutation's spelling.
pub fn each_mutations(source: &str) -> Vec<(&'static str, String)> {
    vec![
        ("each_missing_separator", source.replacen(';', ",", 1)),
        (
            "each_truncated",
            source.trim_end().trim_end_matches(')').to_string(),
        ),
        (
            "each_unknown_operation",
            source.replacen("!each(", "!eacx(", 1),
        ),
        (
            "each_extra_delimiter",
            source.replacen("!each(", "!each(,", 1),
        ),
        ("each_changed_binding", source.replacen("c+", "a+", 1)),
    ]
}

/// Numeric spelling mutations are checked independently of execution outcomes.
pub fn numeric_mutations(source: &str) -> Vec<(&'static str, String)> {
    vec![
        ("numeric_unknown_width", source.replace("u64", "u32")),
        ("numeric_incomplete_fraction", source.replace("1.0", "1.")),
        ("numeric_incomplete_exponent", source.replace("1.0", "1e-")),
        ("numeric_mixed_width", source.replace("1u64", "1u128")),
        (
            "numeric_unknown_conversion",
            source.replace("!bits", "!bitz"),
        ),
    ]
}

/// Nominal declarations, projections, updates and typed maps are mutation tested.
pub fn record_mutations(source: &str) -> Vec<(&'static str, String)> {
    vec![
        ("record_unknown_field", source.replace(".data", ".missing")),
        (
            "record_duplicate_field",
            source.replacen("data:s,buffer:v", "data:s,data:v", 1),
        ),
        (
            "record_unknown_type",
            source.replacen("inner:R", "inner:Missing", 1),
        ),
        (
            "record_missing_update_colon",
            source.replace("{data:", "{data"),
        ),
        (
            "record_dynamic_map_value",
            source.replace("!map[Pair]", "!map[R]"),
        ),
        (
            "record_truncated_declaration",
            source.replacen("count:i)", "count:i", 1),
        ),
    ]
}

/// The same explicit manifest is loaded on every plugin case; no compiler global registry.
pub fn compile_case(source: &str) -> Result<nil_compiler::CompiledProgram, Diagnostic> {
    if source.contains("!plugin(4,") {
        nil_compiler::compile_with_plugins(
            source,
            SourceProfile::ExprV5,
            &[Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fuzz/corpus/expr-v5/std/mirrored.nil-plugin")],
        )
    } else if source.contains("!plugin(3,") || source.contains("!3.") {
        nil_compiler::compile_with_plugins(
            source,
            SourceProfile::ExprV5,
            &[Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fuzz/corpus/expr-v5/higher-order/library.nil-module")],
        )
    } else if source.contains("!plugin(2,") {
        nil_compiler::compile_with_plugins(
            source,
            SourceProfile::ExprV5,
            &[Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fuzz/corpus/expr-v5/modules/application.nil-module")],
        )
    } else if source.contains("!plugin(") {
        nil_compiler::compile_with_plugins(
            source,
            SourceProfile::ExprV5,
            &[Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../plugins/example/plugin.nil-plugin")],
        )
    } else {
        compile_with_profile(source, SourceProfile::ExprV5)
    }
}

pub fn plugin_mutations(source: &str) -> Vec<(&'static str, String)> {
    vec![
        (
            "plugin_unknown_id",
            source.replace("!plugin(1,0,", "!plugin(999,0,"),
        ),
        (
            "plugin_unknown_operation",
            source.replace("!plugin(1,0,", "!plugin(1,999,"),
        ),
        (
            "plugin_noncanonical_id",
            source.replace("!plugin(1,0,", "!plugin(01,0,"),
        ),
        (
            "plugin_missing_separator",
            source.replace("!plugin(1,0,", "!plugin(1 0,"),
        ),
        (
            "plugin_truncated",
            source
                .trim_end_matches('\n')
                .trim_end_matches(')')
                .to_owned(),
        ),
    ]
}

/// Malformed collection spellings and type placements retain deterministic checks.
pub fn collection_mutations(source: &str) -> Vec<(&'static str, String)> {
    vec![
        (
            "collection_wrong_element",
            source.replace("!buffer[Cell]", "!buffer[Row]"),
        ),
        (
            "collection_unknown_type",
            source.replace("v[Cell]", "v[Missing]"),
        ),
        (
            "collection_missing_bracket",
            source.replace("!buffer[Cell]", "!buffer[Cell"),
        ),
        (
            "collection_self_type",
            source.replace(
                "record Cell(text:s,value:i)",
                "record Cell(text:v[Cell],value:i)",
            ),
        ),
        (
            "collection_truncated",
            source[..source.len() / 2].to_string(),
        ),
    ]
}

/// Invalid import graphs are checked before execution, with deterministic E024.
fn module_manifest_mutations(
    seed: u64,
    root: &Path,
    mutations: &mut BTreeMap<&'static str, usize>,
    families: &mut BTreeMap<&'static str, usize>,
) -> Result<(), String> {
    let folder = root.join("module-manifest-mutations");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    fs::write(folder.join("provider.nil"), format!("1=a+{}", seed % 100))
        .map_err(|e| e.to_string())?;
    let base = "nil-module 1\nid 7\nsource provider.nil\nexport 0 0\n";
    fs::write(
        folder.join("cycle"),
        "nil-module 1\nid 8\nsource provider.nil\nexport 0 0\nimport manifest\n",
    )
    .map_err(|e| e.to_string())?;
    fs::write(folder.join("duplicate"), base).map_err(|e| e.to_string())?;
    for (family, manifest, source) in [
        (
            "module_duplicate_namespace",
            format!("{base}import duplicate\n"),
            "=42",
        ),
        (
            "module_missing_export",
            base.to_string(),
            "=!plugin(7,9,42)",
        ),
        (
            "module_duplicate_export",
            format!("{base}export 0 0\n"),
            "=42",
        ),
        (
            "module_missing_import",
            format!("{base}import absent\n"),
            "=42",
        ),
        (
            "module_duplicate_import",
            format!("{base}import cycle\nimport cycle\n"),
            "=42",
        ),
        (
            "module_import_cycle",
            format!("{base}import cycle\n"),
            "=42",
        ),
        (
            "module_unknown_version",
            base.replace("nil-module 1", "nil-module 2"),
            "=42",
        ),
        (
            "module_noncanonical_namespace",
            base.replace("id 7", "id 07"),
            "=42",
        ),
        (
            "module_missing_function",
            base.replace("export 0 0", "export 0 99"),
            "=42",
        ),
    ] {
        fs::write(folder.join("manifest"), manifest).map_err(|e| e.to_string())?;
        let check = || {
            nil_compiler::compile_with_plugins(
                source,
                SourceProfile::ExprV5,
                &[folder.join("manifest")],
            )
            .unwrap_err()
        };
        let a = check();
        let b = check();
        if a != b || a.code != "E024" {
            return Err(format!("module mutation divergence: {family}: {a}"));
        }
        *mutations.entry(family).or_default() += 1;
        *families.entry(family).or_default() += 1;
    }
    fs::remove_dir_all(folder).map_err(|e| e.to_string())?;
    Ok(())
}

/// Resource classification must never conceal signals, sanitizer output or malformed diagnostics.
fn native_outcome(output: &std::process::Output) -> Result<&str, String> {
    if output.status.success() {
        return if output.stderr.is_empty() {
            Ok("OK")
        } else {
            Err("unexpected stderr on success".into())
        };
    }
    if output.status.code() != Some(1) {
        return Err(format!("native process failure: {}", output.status));
    }
    let text =
        std::str::from_utf8(&output.stderr).map_err(|_| "invalid native diagnostic encoding")?;
    let mut words = text.split_whitespace();
    let code = words.next().unwrap_or("");
    let location_or_message = words.next().unwrap_or("");
    let valid_location = if location_or_message.starts_with('@') {
        location_or_message
            .strip_prefix('@')
            .and_then(|s| s.split_once(".."))
            .and_then(|(a, b)| Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?)))
            .is_some_and(|(a, b)| a <= b)
            && words.next().is_some()
    } else {
        !location_or_message.is_empty()
    };
    if code.len() != 4
        || !code.starts_with('E')
        || !code[1..].bytes().all(|b| b.is_ascii_digit())
        || !valid_location
        || text.lines().count() != 1
    {
        return Err(format!("malformed native diagnostic: {text}"));
    }
    Ok(code)
}
fn persistent_resource_direction(
    original_reference_ok: bool,
    reference: &str,
    native: &str,
) -> Result<&'static str, String> {
    if original_reference_ok && native == "E013" {
        return Err("native E013 persists after successful original reference".into());
    }
    Ok(resource_direction(reference, native))
}
fn resource_direction(reference: &str, native: &str) -> &'static str {
    match (
        matches!(reference, "E008" | "E013"),
        matches!(native, "E008" | "E013"),
    ) {
        (true, true) => "both",
        (true, false) => "reference-only",
        (false, true) => "native-only",
        (false, false) => "neither",
    }
}
#[cfg(all(test, unix))]
mod resource_tests {
    use super::native_outcome;
    use std::{
        os::unix::process::ExitStatusExt,
        process::{ExitStatus, Output},
    };
    fn output(status: i32, stderr: &[u8]) -> Output {
        Output {
            status: ExitStatus::from_raw(status),
            stdout: vec![],
            stderr: stderr.into(),
        }
    }
    #[test]
    fn native_memory_failure_after_original_reference_success_is_not_inconclusive() {
        assert!(super::persistent_resource_direction(true, "OK", "E013").is_err());
        assert!(super::persistent_resource_direction(true, "E008", "E013").is_err());
        assert_eq!(
            super::persistent_resource_direction(true, "OK", "E008").unwrap(),
            "native-only"
        );
        assert_eq!(
            super::persistent_resource_direction(false, "E013", "E013").unwrap(),
            "both"
        );
    }
    #[test]
    fn persistent_resource_directions_are_separate() {
        assert_eq!(super::resource_direction("OK", "E008"), "native-only");
        assert_eq!(super::resource_direction("E013", "OK"), "reference-only");
        assert_eq!(super::resource_direction("E008", "E013"), "both");
        assert_eq!(super::resource_direction("E014", "E014"), "neither");
    }
    #[test]
    fn clean_resource_failure_is_classified_without_equating_its_span() {
        for text in [
            b"E013 @3..4 guard exceeded\n".as_slice(),
            b"E013 argument allocation failed\n",
        ] {
            assert_eq!(native_outcome(&output(256, text)).unwrap(), "E013");
        }
    }
    #[test]
    fn resource_text_does_not_hide_crashes_sanitizers_or_malformed_spans() {
        for out in [
            output(6, b"E013 @3..4 guard exceeded\n"),
            output(256, b"E013 @bad guard exceeded\n"),
            output(256, b"E013 @4..3 guard exceeded\n"),
            output(
                256,
                b"E013 @3..4 guard exceeded\nAddressSanitizer: heap-buffer-overflow\n",
            ),
            output(0, b"runtime error: overflow\n"),
        ] {
            assert!(native_outcome(&out).is_err());
        }
    }
}
