//! LLVM AOT prototype for validated, syntax-independent HIR. No unsafe Rust/FFI.
mod bulk_compare;
mod emit;
mod linear_inline;
mod loop_storage;
mod ownership;
mod read_range;
mod runtime;
pub use emit::emit_llvm_with_copy_hints;
pub use emit::{emit_llvm, emit_llvm_with_instrumentation};
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
/// Resource accounting is optional tooling, separate from integer semantics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Instrumentation {
    #[default]
    ProfileDefault,
    Bounded,
    Unbounded,
}
impl Instrumentation {
    pub fn bounded(self, program: &ValidatedProgram) -> bool {
        match self {
            Self::ProfileDefault => program.program().arithmetic == nil_hir::Arithmetic::Checked,
            Self::Bounded => true,
            Self::Unbounded => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub explain_copies: bool,
    pub instrumentation: Instrumentation,
    pub entry: FunctionId,
    pub optimization: Optimization,
    pub steps: u64,
    pub call_depth: u64,
    /// Managed-arena safety guard; zero disables it. Not a semantic quota.
    pub memory_bytes: u64,
    /// Usable guarded native worker stack, in bytes (minimum 64 KiB).
    pub stack_bytes: u64,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            explain_copies: false,
            instrumentation: Instrumentation::ProfileDefault,
            entry: FunctionId(0),
            optimization: Optimization::O2,
            steps: 100_000,
            call_depth: 16_384,
            memory_bytes: nil_hir::DEFAULT_MEMORY_BYTES as u64,
            stack_bytes: 32 * 1024 * 1024,
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
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
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
/// Render the actual C entry driver, including flattening and output formatting.
/// Benchmarks can reuse it with a matching foreign entry bridge.
pub fn entry_runtime(program: &ValidatedProgram, options: Options) -> Result<String, Diagnostic> {
    let entry = program
        .program()
        .functions
        .get(options.entry.0)
        .ok_or_else(|| {
            Diagnostic::new(
                "E004",
                Phase::Backend,
                None,
                "unknown native entry function",
            )
        })?;
    if std::iter::once(&entry.result_type)
        .chain(&entry.parameters)
        .any(|ty| {
            matches!(
                ty,
                Type::Record(..) | Type::MapRecord(..) | Type::RecordBuffer(..)
            )
        })
    {
        return Err(Diagnostic::new(
            "E010",
            Phase::Backend,
            None,
            "record native entry requires a scalar/sequence wrapper",
        ));
    }
    let driver = if emit::uses_application(program) {
        runtime::application_source(&options, entry, emit::uses_record_buffers(program))
    } else if entry.result_type != Type::I64 || entry.parameters.iter().any(|t| *t != Type::I64) {
        runtime::typed_source(&options, entry)
    } else {
        runtime::source(&options, entry.parameters.len())
    };
    if options.stack_bytes < 64 * 1024 || options.stack_bytes > usize::MAX as u64 - 65536 {
        return Err(error(
            "native stack size must be at least 64 KiB and fit the host",
        ));
    }
    let mut driver = driver.replacen("int main(", "static int nil_program_main(", 1);
    driver.push_str(
        &include_str!("stack.c").replace("$STACK_BYTES", &options.stack_bytes.to_string()),
    );
    Ok(driver)
}

/// Build for the host. Stage output privately and publish only after successful
/// LLVM verification, code generation and linking; failed builds preserve outputs.
pub fn build(
    program: &ValidatedProgram,
    output: &Path,
    options: Options,
) -> Result<BuildStats, Diagnostic> {
    build_internal(program, output, options, None)
}
/// Differential-testing driver: host stdout is unchanged; the successful return
/// value is rendered into `receipt`. The receipt path must be absolute.
pub fn build_with_result_receipt(
    program: &ValidatedProgram,
    output: &Path,
    options: Options,
    receipt: &Path,
) -> Result<BuildStats, Diagnostic> {
    if !receipt.is_absolute() {
        return Err(error("result receipt must be an absolute path"));
    }
    build_internal(program, output, options, Some(receipt))
}
fn build_internal(
    program: &ValidatedProgram,
    output: &Path,
    options: Options,
    receipt: Option<&Path>,
) -> Result<BuildStats, Diagnostic> {
    if !cfg!(all(
        any(target_os = "macos", target_os = "linux"),
        any(target_arch = "aarch64", target_arch = "x86_64")
    )) {
        return Err(error(
            "guarded native builds currently support aarch64/x86_64 macOS/Linux hosts",
        ));
    }
    let entry = program
        .program()
        .functions
        .get(options.entry.0)
        .ok_or_else(|| {
            Diagnostic::new(
                "E004",
                Phase::Backend,
                None,
                "unknown native entry function",
            )
        })?;
    if std::iter::once(&entry.result_type)
        .chain(&entry.parameters)
        .any(|ty| {
            matches!(
                ty,
                Type::Record(..) | Type::MapRecord(..) | Type::RecordBuffer(..)
            )
        })
    {
        return Err(Diagnostic::new(
            "E010",
            Phase::Backend,
            None,
            "record native entry requires a scalar/sequence wrapper",
        ));
    }
    let typed_entry = entry.result_type != Type::I64
        || entry.parameters.iter().any(|t| *t != Type::I64)
        || emit::uses_application(program);
    let total = Instant::now();
    let start = Instant::now();
    let mut llvm =
        emit_llvm_with_copy_hints(program, options.instrumentation, options.explain_copies);
    if typed_entry {
        llvm.push_str(&emit::entry_bridge(program, options.entry));
    }
    let probe = if cfg!(target_os = "macos") {
        "__chkstk_darwin"
    } else {
        "inline-asm"
    };
    llvm = llvm
        .lines()
        .map(|line| {
            if line.starts_with("define ") && line.ends_with("{") {
                format!(
                    "{} \"probe-stack\"=\"{}\" {{\n",
                    line.trim_end_matches('{').trim_end(),
                    probe
                )
            } else {
                format!("{line}\n")
            }
        })
        .collect();
    let mut runtime = entry_runtime(program, options)?;
    if let Some(receipt) = receipt {
        if !emit::uses_application(program) {
            return Err(error("result receipt requires an application driver"));
        }
        let path = receipt
            .as_os_str()
            .as_encoded_bytes()
            .iter()
            .map(|byte| format!("\\x{byte:02x}"))
            .collect::<String>();
        let marker = "nil_entry(&ctx,values,result);\n";
        if !runtime.contains(marker) {
            return Err(error("application result receipt marker missing"));
        }
        runtime = runtime.replace(marker, &format!(
            "{marker}fflush(stdout); FILE *nil_receipt=fopen(\"{path}\",\"wb\");\nif(!nil_receipt || dup2(fileno(nil_receipt),STDOUT_FILENO)<0) {{ fputs(\"E011 result receipt failed\\n\",stderr);exit(1); }}\nfclose(nil_receipt);\n"
        ));
    }
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
    // Application O2 uses whole-program optimization so checked C accessors
    // can inline without duplicating their semantics in the LLVM emitter.
    let lto = if emit::uses_application(program) && matches!(options.optimization, Optimization::O2)
    {
        vec!["-flto"]
    } else {
        vec![]
    };
    let llvm_codegen_ns = invoke(
        Command::new(clang())
            .args(&lto)
            .args([
                "-fno-fast-math",
                "-fno-associative-math",
                "-fno-reciprocal-math",
                "-ffp-contract=off",
            ])
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
            .args(&lto)
            .args([
                "-fno-fast-math",
                "-fno-associative-math",
                "-fno-reciprocal-math",
                "-ffp-contract=off",
            ])
            .arg(if cfg!(target_os = "macos") {
                "-fstack-check"
            } else {
                "-fstack-clash-protection"
            })
            .args(["-pthread", "-std=c11", options.optimization.flag(), "-c"])
            .arg(&support)
            .arg("-o")
            .arg(&runtime_object),
    )?;
    let linker: &[&str] = if cfg!(target_os = "linux") && !lto.is_empty() {
        &["-fuse-ld=lld"]
    } else {
        &[]
    };
    // LLVM may lower correctly rounded scalar math to platform libm calls,
    // particularly at O0 or on CPUs without an integral-rounding instruction.
    let math_library: &[&str] = if cfg!(target_os = "linux") {
        &["-lm"]
    } else {
        &[]
    };
    let link_ns = invoke(
        Command::new(clang())
            .args(&lto)
            .args([
                "-fno-fast-math",
                "-fno-associative-math",
                "-fno-reciprocal-math",
                "-ffp-contract=off",
            ])
            .args(linker)
            .arg(options.optimization.flag())
            .arg(&object)
            .arg(&runtime_object)
            .args(math_library)
            .arg("-pthread")
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
    run_arguments(
        program,
        options,
        &arguments.iter().map(i64::to_string).collect::<Vec<_>>(),
    )
}

pub fn run_arguments(
    program: &ValidatedProgram,
    options: Options,
    arguments: &[String],
) -> Result<std::process::Output, Diagnostic> {
    let entry = program
        .program()
        .functions
        .get(options.entry.0)
        .ok_or_else(|| Diagnostic::new("E004", Phase::Execute, None, "unknown entry function"))?;
    let slots: usize = entry.parameters.iter().map(|ty| ty.slots()).sum();
    if arguments.len() != slots {
        return Err(
            Diagnostic::new("E006", Phase::Execute, None, "entry arity mismatch")
                .mismatch(slots, arguments.len()),
        );
    }
    let temporary = Temporary::create(&std::env::temp_dir())?;
    let binary = temporary.0.join("program");
    build(program, &binary, options)?;
    Command::new(&binary)
        .args(arguments)
        .output()
        .map_err(|e| error(format!("cannot execute native program: {e}")))
}
