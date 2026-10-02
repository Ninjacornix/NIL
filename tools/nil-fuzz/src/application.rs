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
        Value::I64(v) => format!("{v}\n").into_bytes(),
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
        let signature = "(v,s,s,s)";
        let expression = match mode % 56 {
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
            _ => format!("=#!slice(!concat(!buffer({n},{v}),a),{n},#a)+#b"),
        };
        Self {
            source: format!("{signature}{expression}\n"),
            denied: matches!(mode % 56, 20..=22 | 35),
        }
    }
}

/// Keeps each failed source, input, IR and binary for reproduction.
pub fn campaign(seed: u64, cases: usize, root: &Path) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut codes = BTreeMap::<String, usize>::new();
    let mut operations = BTreeSet::new();
    for index in 0..cases {
        let case_seed = seed.wrapping_add(index as u64);
        let case = Case::new(case_seed, index);
        let folder = root.join(format!("v5-{case_seed}"));
        fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        fs::write(folder.join("source.nil"), &case.source).map_err(|e| e.to_string())?;
        fs::write(
            folder.join("replay.txt"),
            format!("seed={seed} index={index} denied={}\n", case.denied),
        )
        .map_err(|e| e.to_string())?;
        let compiled = compile_with_profile(&case.source, SourceProfile::ExprV5)
            .map_err(|e| format!("{}: {e}", folder.display()))?;
        fs::write(folder.join("module.ll"), nil_llvm::emit_llvm(&compiled.hir))
            .map_err(|e| e.to_string())?;
        // These count generated syntax coverage; errors below count executed outcomes.
        for op in [
            "buffer", "bytes", "concat", "slice", "format", "parse", "read", "write", "out",
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
                    index % 48,
                    opt.flag(),
                    folder.display()
                ));
            }
        }
        fs::remove_dir_all(folder).map_err(|e| e.to_string())?;
    }
    let operations = operations.into_iter().collect::<Vec<_>>();
    println!(
        "{{\"seed\":{seed},\"programs\":{cases},\"native_builds\":{},\"divergences\":0,\"observed_codes\":{codes:?},\"generated_intrinsics\":{operations:?}}}",
        cases * 2
    );
    Ok(())
}
