//! LLVM AOT prototype for validated, syntax-independent HIR. No unsafe Rust/FFI.
mod emit;
mod runtime;
pub use emit::emit_llvm;
use nil_hir::{Diagnostic, FunctionId, Phase, Type, ValidatedProgram};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

#[derive(Clone, Copy, Debug)]
pub enum Optimization {
    O0,
    O2,
}
impl Optimization {
    pub fn flag(self) -> &'static str {
        match self {
            Self::O0 => "-O0",
            Self::O2 => "-O2",
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub entry: FunctionId,
    pub optimization: Optimization,
    pub steps: u64,
    pub call_depth: u64,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            entry: FunctionId(0),
            optimization: Optimization::O2,
            steps: 100_000,
            call_depth: 256,
        }
    }
}
#[derive(Debug)]
pub struct BuildStats {
    pub ir_lowering_ns: u128,
    pub llvm_codegen_ns: u128,
    pub runtime_compile_ns: u128,
    pub link_ns: u128,
    pub total_ns: u128,
    pub binary_bytes: u64,
}
fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("E011", Phase::Backend, None, message)
}
struct Temporary(PathBuf);
impl Temporary {
    fn create(parent: &Path) -> Result<Self, Diagnostic> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let path = parent.join(format!(
                ".nil-llvm-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(error(format!("cannot create build directory: {e}"))),
            }
        }
        Err(error("cannot allocate build directory"))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn clang() -> OsString {
    std::env::var_os("NIL_CLANG").unwrap_or_else(|| "clang".into())
}
fn invoke(command: &mut Command) -> Result<u128, Diagnostic> {
    let start = Instant::now();
    let output = command.output().map_err(|e| {
        error(format!(
            "cannot execute Clang; install Clang 15+ or set NIL_CLANG: {e}"
        ))
    })?;
    if !output.status.success() {
        return Err(error(format!(
            "Clang failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(start.elapsed().as_nanos())
}
/// Build for the host. Stage output privately and publish only after successful
/// LLVM verification, code generation and linking; failed builds preserve outputs.
pub fn build(
    program: &ValidatedProgram,
    output: &Path,
    options: Options,
) -> Result<BuildStats, Diagnostic> {
    if !cfg!(any(target_os = "macos", target_os = "linux")) || std::mem::size_of::<usize>() != 8 {
        return Err(error(
            "native builds currently support 64-bit macOS/Linux hosts",
        ));
    }
    let entry = program
        .program()
        .functions
        .get(options.entry.0)
        .ok_or_else(|| error("unknown native entry function"))?;
    if entry.result_type != Type::I64 || entry.parameters.iter().any(|t| *t != Type::I64) {
        return Err(error("native CLI entry requires i64 parameters and result"));
    }
    if options.call_depth > 256 {
        return Err(error("native call depth must be 0..256"));
    }
    let total = Instant::now();
    let start = Instant::now();
    let llvm = emit_llvm(program);
    let runtime = runtime::source(&options, entry.parameters.len());
    let ir_lowering_ns = start.elapsed().as_nanos();
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let temporary = Temporary::create(parent)?;
    let module = temporary.0.join("module.ll");
    let support = temporary.0.join("runtime.c");
    let object = temporary.0.join("module.o");
    let runtime_object = temporary.0.join("runtime.o");
    let executable = temporary.0.join("program");
    fs::write(&module, llvm)
        .and_then(|()| fs::write(&support, runtime))
        .map_err(|e| error(format!("cannot write backend input: {e}")))?;
    let llvm_codegen_ns = invoke(
        Command::new(clang())
            .args([
                options.optimization.flag(),
                "-Wno-override-module",
                "-x",
                "ir",
                "-c",
            ])
            .arg(&module)
            .arg("-o")
            .arg(&object),
    )?;
    let runtime_compile_ns = invoke(
        Command::new(clang())
            .args(["-std=c11", options.optimization.flag(), "-c"])
            .arg(&support)
            .arg("-o")
            .arg(&runtime_object),
    )?;
    let link_ns = invoke(
        Command::new(clang())
            .arg(&object)
            .arg(&runtime_object)
            .arg("-o")
            .arg(&executable),
    )?;
    let binary_bytes = fs::metadata(&executable)
        .map_err(|e| error(e.to_string()))?
        .len();
    fs::rename(&executable, output)
        .map_err(|e| error(format!("cannot publish executable: {e}")))?;
    Ok(BuildStats {
        ir_lowering_ns,
        llvm_codegen_ns,
        runtime_compile_ns,
        link_ns,
        total_ns: total.elapsed().as_nanos(),
        binary_bytes,
    })
}

/// Compile and execute in a private temporary directory. There is no interpreter
/// fallback: toolchain failures are explicit backend diagnostics.
pub fn run(
    program: &ValidatedProgram,
    options: Options,
    arguments: &[i64],
) -> Result<std::process::Output, Diagnostic> {
    let entry = program
        .program()
        .functions
        .get(options.entry.0)
        .ok_or_else(|| Diagnostic::new("E004", Phase::Execute, None, "unknown entry function"))?;
    if arguments.len() != entry.parameters.len() {
        return Err(
            Diagnostic::new("E006", Phase::Execute, None, "entry arity mismatch")
                .mismatch(entry.parameters.len(), arguments.len()),
        );
    }
    let temporary = Temporary::create(&std::env::temp_dir())?;
    let binary = temporary.0.join("program");
    build(program, &binary, options)?;
    Command::new(&binary)
        .args(arguments.iter().map(i64::to_string))
        .output()
        .map_err(|e| error(format!("cannot execute native program: {e}")))
}
