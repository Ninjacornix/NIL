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
#[derive(Default)]
struct MemoryHost {
    files: BTreeMap<Vec<u8>, Vec<u8>>,
    stdout: Vec<u8>,
    writable: Vec<u8>,
    denied: bool,
}
impl Host for MemoryHost {
    fn read(&mut self, path: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        if self.denied {
            return Err(fault("E018"));
        }
        self.files.get(path).cloned().ok_or_else(|| fault("E015"))
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
        Value::Record(..) | Value::RecordBuffer(..) => {
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
        let mut r = Random(seed);
        let n = [0, 1, 2, 7, 31, 257][r.pick(6)];
        let v = r.integer();
        let byte = r.pick(256);
        let x = r.next_u64();
        let y = r.next_u64();
        let signature = "(v,s,s,s)";
        let expression = match mode % 308 {
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
            _ => format!("=#!slice(!concat(!buffer({n},{v}),a),{n},#a)+#b"),
        };
        Self {
            source: format!(
                "{}{signature}{expression}\n",
                if mode % 308 >= 264 {
                    "record Zero(empty:0)\nrecord ZeroMany(left:0,right:0)\nrecord ZeroNested(inner:Zero,more:ZeroMany)\nrecord ZeroMixed(zero:ZeroNested,value:i,empty:0)\nrecord ZeroArray(empty:0,full:1)\nrecord Cell(text:s,value:i)\nrecord Row(cells:v[Cell],data:v,dict:m)\nrecord Packet(data:s,buffer:v,count:i,rows:v[Row])\n"
                } else if mode % 308 >= 240 {
                    "record Packet(data:s,buffer:v,count:i)\n"
                } else if mode % 308 >= 208 {
                    "record R(data:s,buffer:v,count:i)\nrecord Nested(inner:R)\nrecord Scalar(value:i,bits:f64,wide:u128,flag:b)\nrecord Pair(left:i,right:i)\n"
                } else {
                    ""
                }
            ),
            denied: matches!(mode % 308, 20..=22 | 35),
        }
    }
}

/// Keeps each failed source, input, IR and binary for reproduction.
pub fn campaign(seed: u64, cases: usize, root: &Path) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut codes = BTreeMap::<String, usize>::new();
    let mut operations = BTreeSet::new();
    let mut source_mutations = BTreeMap::<&str, usize>::new();
    let mut families = BTreeMap::<&str, usize>::new();
    for index in 0..cases {
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
        ]
        .get((index % 308).wrapping_sub(64))
        {
            *families.entry(name).or_default() += 1;
        }
        let case_seed = seed.wrapping_add(index as u64);
        let case = Case::new(case_seed, index);
        if index % 308 >= 264 {
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
        if index % 308 >= 240 {
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
        if (168..208).contains(&(index % 308)) {
            for (name, mutated) in numeric_mutations(&case.source) {
                if mutated == case.source {
                    continue;
                }
                crate::frontend_with_profile(&mutated, SourceProfile::ExprV5);
                *source_mutations.entry(name).or_default() += 1;
            }
        }
        if (208..240).contains(&(index % 308)) {
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
        let compiled =
            compile_case(&case.source).map_err(|e| format!("{}: {e}", folder.display()))?;
        fs::write(folder.join("module.ll"), nil_llvm::emit_llvm(&compiled.hir))
            .map_err(|e| e.to_string())?;
        // These count generated syntax coverage; errors below count executed outcomes.
        for op in [
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
        let input = folder.join("input");
        let output = folder.join("output");
        let mut r = Random(case_seed ^ 0xa11a5);
        let buffer = (0..1 + r.pick(16)).map(|_| r.integer()).collect::<Vec<_>>();
        let text = (0..1 + r.pick(16))
            .map(|_| b' ' + r.pick(95) as u8)
            .collect::<Vec<_>>();
        let file = (0..r.pick(10000))
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
            String::from_utf8(text).unwrap(),
            input.to_str().unwrap().into(),
            output.to_str().unwrap().into(),
        ];
        fs::write(folder.join("arguments.txt"), format!("{native_args:?}\n"))
            .map_err(|e| e.to_string())?;
        let mut host = MemoryHost {
            files: BTreeMap::from([
                (input_bytes, file.clone()),
                (output_bytes.clone(), b"sentinel".to_vec()),
            ]),
            writable: output_bytes.clone(),
            denied: case.denied,
            ..Default::default()
        };
        let expected = execute_values_with_host(
            &compiled.hir,
            FunctionId(0),
            &args,
            Limits {
                steps: 1_000_000,
                ..Limits::default()
            },
            &mut host,
        );
        let code = expected.as_ref().err().map(|e| e.code).unwrap_or("OK");
        *codes.entry(code.into()).or_default() += 1;
        let expected_stdout = match &expected {
            Ok(v) => [host.stdout.as_slice(), render(v).as_slice()].concat(),
            Err(_) => host.stdout.clone(),
        };
        for opt in [Optimization::O0, Optimization::O2] {
            fs::write(&input, &file).map_err(|e| e.to_string())?;
            fs::write(&output, b"sentinel").map_err(|e| e.to_string())?;
            let binary = folder.join(format!("native-{}", opt.flag()));
            nil_llvm::build(
                &compiled.hir,
                &binary,
                Options {
                    optimization: opt,
                    ..Default::default()
                },
            )
            .map_err(|e| e.to_string())?;
            let mut command = Command::new(binary);
            command.args(&native_args).env_remove("NIL_DENY_HOST_IO");
            if case.denied {
                command.env("NIL_DENY_HOST_IO", "1");
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
            let deadline = Instant::now() + Duration::from_secs(10);
            let status = loop {
                if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                    break status;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("native timeout: {}", folder.display()));
                }
                std::thread::sleep(Duration::from_millis(5));
            };
            let actual = std::process::Output {
                status,
                stdout: fs::read(stdout_path).map_err(|e| e.to_string())?,
                stderr: fs::read(stderr_path).map_err(|e| e.to_string())?,
            };
            fs::write(folder.join("stdout.bin"), &actual.stdout).map_err(|e| e.to_string())?;
            fs::write(folder.join("stderr.txt"), &actual.stderr).map_err(|e| e.to_string())?;
            let actual_code = if actual.status.success() {
                "OK"
            } else {
                std::str::from_utf8(&actual.stderr)
                    .unwrap_or("")
                    .split_whitespace()
                    .next()
                    .unwrap_or("NO_DIAGNOSTIC")
            };
            if actual_code != code
                || actual.stdout != expected_stdout
                || fs::read(&output).map_err(|e| e.to_string())? != host.files[&output_bytes]
                || fs::read(&input).map_err(|e| e.to_string())? != file
            {
                return Err(format!(
                    "divergence seed={case_seed} mode={} {} expected={code} native={actual_code}; {}",
                    index % 308,
                    opt.flag(),
                    folder.display()
                ));
            }
        }
        fs::remove_dir_all(folder).map_err(|e| e.to_string())?;
    }
    let operations = operations.into_iter().collect::<Vec<_>>();
    println!(
        "{{\"seed\":{seed},\"programs\":{cases},\"native_builds\":{},\"divergences\":0,\"observed_codes\":{codes:?},\"generated_intrinsics\":{operations:?},\"call_families\":{families:?},\"source_mutations\":{source_mutations:?}}}",
        cases * 2
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
    if source.contains("!plugin(") {
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
