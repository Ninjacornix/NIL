use nil_compiler::{SourceProfile, compile_with_profile, evaluator::execute};
use nil_fuzz::{
    CORPUS, LIMITS,
    generate::{Case, Random},
};
use nil_hir::FunctionId;
use nil_llvm::{Instrumentation, Optimization, Options};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    thread,
    time::{Duration, Instant},
};

fn wait(command: &mut Command, limit: Duration) -> Result<std::process::ExitStatus, String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Ok(status);
        }
        if start.elapsed() > limit {
            #[cfg(unix)]
            {
                // Worker and all Clang/native descendants share this newly created group.
                let _ = Command::new("/bin/kill")
                    .args(["-KILL", "--", &format!("-{}", child.id())])
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("worker timeout after {}s", limit.as_secs()));
        }
        thread::sleep(Duration::from_millis(5));
    }
}
fn worker(seed: u64, out: &Path) -> Result<(), String> {
    let case = Case::new(seed);
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    fs::write(out.join("source.nil"), &case.source).map_err(|e| e.to_string())?;
    nil_fuzz::valid(&case);
    let p = compile_with_profile(&case.source, SourceProfile::ExprV3).map_err(|e| e.to_string())?;
    for entry in 0..3 {
        let args = if entry == 2 {
            vec![vec![0], vec![1], vec![5]]
        } else {
            case.arguments.clone()
        };
        for optimization in [Optimization::O0, Optimization::O2] {
            for instrumentation in [Instrumentation::Bounded, Instrumentation::Unbounded] {
                let options = Options {
                    entry: FunctionId(entry),
                    optimization,
                    instrumentation,
                    steps: LIMITS.steps,
                    call_depth: LIMITS.call_depth as u64,
                };
                let binary = out.join("program");
                fs::write(out.join("mode.txt"),format!("seed={seed}\nentry={entry}\noptimization={optimization:?}\ninstrumentation={instrumentation:?}\n")).map_err(|e|e.to_string())?;
                fs::write(
                    out.join("module.ll"),
                    nil_llvm::emit_llvm_with_instrumentation(&p.hir, instrumentation),
                )
                .map_err(|e| e.to_string())?;
                nil_llvm::build(&p.hir, &binary, options).map_err(|e| e.to_string())?;
                for args in &args {
                    fs::write(out.join("arguments.txt"), format!("{args:?}\n"))
                        .map_err(|e| e.to_string())?;
                    let oracle = case.expected_function(entry, args);
                    let reference = execute(&p.hir, FunctionId(entry), args, LIMITS);
                    match (&oracle, &reference) {
                        (Ok(a), Ok(b)) if a == b => {}
                        (Err("division by zero"), Err(e))
                            if e.code == "E009" && e.message == "division by zero" => {}
                        _ => {
                            return Err(format!(
                                "oracle/reference mismatch: {oracle:?} {reference:?}"
                            ));
                        }
                    }
                    let actual = Command::new(&binary)
                        .args(args.iter().map(i64::to_string))
                        .output()
                        .map_err(|e| e.to_string())?;
                    match reference {
                        Ok(v)
                            if actual.status.success()
                                && actual.stdout == format!("{v}\n").as_bytes() => {}
                        Err(e)
                            if actual.status.code() == Some(1)
                                && actual.stdout.is_empty()
                                && actual.stderr == format!("{e}\n").as_bytes() => {}
                        expected => {
                            return Err(format!(
                                "native mismatch: expected={expected:?} actual={actual:?}"
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
fn input(bytes: &[u8], seed: u64, phase: &str, out: &Path) -> Result<bool, String> {
    fs::write(out.join("current-input.bin"), bytes).map_err(|e| e.to_string())?;
    fs::write(
        out.join("current.txt"),
        format!("seed={seed}\nphase={phase}\n"),
    )
    .map_err(|e| e.to_string())?;
    let Ok(source) = std::str::from_utf8(bytes) else {
        return Ok(false);
    };
    nil_fuzz::frontend(source);
    Ok(true)
}
fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|a| a == "--worker") {
        if args.len() != 3 {
            return Err("usage: --worker SEED OUTPUT".into());
        }
        return worker(
            args[1].parse().map_err(|_| "invalid seed")?,
            Path::new(&args[2]),
        );
    }
    if args.first().is_some_and(|a| a == "--timeout-probe") {
        thread::sleep(Duration::from_secs(30));
        return Ok(());
    }
    let mut cases = 1000usize;
    let mut seed = 5130572u64;
    let mut native = 8usize;
    let mut out = PathBuf::from("fuzz/artifacts");
    let mut replay = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--help" {
            println!(
                "nil-fuzz [--cases N] [--seed U64] [--native N] [--out DIR] [--replay FILE]\nDefaults: 1000 cases, seed 5130572, 8 native cases. Native requires host Clang. Use --native 0 for frontend-only."
            );
            return Ok(());
        }
        let value = it.next().ok_or("missing flag value")?;
        match flag.as_str() {
            "--cases" => cases = value.parse().map_err(|_| "invalid cases")?,
            "--seed" => seed = value.parse().map_err(|_| "invalid seed")?,
            "--native" => native = value.parse().map_err(|_| "invalid native count")?,
            "--out" => out = value.into(),
            "--replay" => replay = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if cases > 1000000 || native > cases {
        return Err("require cases<=1000000 and native<=cases".into());
    }
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let start = Instant::now();
    if let Some(path) = replay {
        input(
            &fs::read(path).map_err(|e| e.to_string())?,
            seed,
            "replay",
            &out,
        )?;
        println!("replay passed");
        return Ok(());
    }
    for source in CORPUS {
        input(source.as_bytes(), seed, "corpus", &out)?;
    }
    let mut utf8 = 0;
    let mut rejected_utf8 = 0;
    for index in 0..cases {
        let case_seed = seed.wrapping_add(index as u64);
        let case = Case::new(case_seed);
        let mut r = Random(case_seed ^ 0xfeedface);
        input(case.source.as_bytes(), case_seed, "valid", &out)?;
        nil_fuzz::valid(&case);
        for _ in 0..4 {
            let mutated = nil_fuzz::mutate(case.source.as_bytes(), &mut r);
            if input(&mutated, case_seed, "mutation", &out)? {
                utf8 += 1;
            } else {
                rejected_utf8 += 1;
            }
        }
        let raw = (0..r.pick(512))
            .map(|_| r.next_u64() as u8)
            .collect::<Vec<_>>();
        if input(&raw, case_seed, "raw bytes", &out)? {
            utf8 += 1;
        } else {
            rejected_utf8 += 1;
        }
        input(case.source.as_bytes(), case_seed, "hir", &out)?;
        nil_fuzz::hir(&case, &mut r);
        if index < native {
            let folder = out.join(format!("native-{case_seed}"));
            fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            let log = fs::File::create(folder.join("worker.log")).map_err(|e| e.to_string())?;
            let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
            command
                .arg("--worker")
                .arg(case_seed.to_string())
                .arg(&folder)
                .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
                .stderr(Stdio::from(log));
            match wait(&mut command, Duration::from_secs(30)) {
                Ok(status) if status.success() => {
                    fs::remove_dir_all(folder).map_err(|e| e.to_string())?
                }
                status => {
                    return Err(format!(
                        "native seed {case_seed}: {status:?}; artifacts at {}",
                        folder.display()
                    ));
                }
            }
        }
        if index % 1000 == 999 {
            println!("checked {} cases", index + 1);
        }
    }
    println!(
        "{{\"seed\":{seed},\"valid_cases\":{cases},\"mutations\":{},\"hir_mutations\":{cases},\"utf8_inputs\":{utf8},\"invalid_utf8\":{rejected_utf8},\"native_cases\":{native},\"native_builds\":{},\"elapsed_ms\":{}}}",
        cases * 5,
        native * 12,
        start.elapsed().as_millis()
    );
    Ok(())
}
fn main() -> ExitCode {
    match std::panic::catch_unwind(run) {
        Ok(Ok(())) => ExitCode::SUCCESS,
        Ok(Err(e)) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
        Err(_) => {
            eprintln!(
                "fuzz property failed; current-input.bin/current.txt or native worker directory preserve the reproducer"
            );
            ExitCode::FAILURE
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeout_kills_and_reaps_worker() {
        let mut command = Command::new("sleep");
        command.arg("30");
        let start = Instant::now();
        assert!(wait(&mut command, Duration::from_millis(30)).is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
