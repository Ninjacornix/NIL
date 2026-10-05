use nil_compiler::{
    SourceProfile, compile_with_plugins, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use nil_hir::FunctionId;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "nil-module-check-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }
    fn module(&self, id: u32, body: &str, extra: &str) -> PathBuf {
        self.file(&format!("{id}.nil"), body);
        self.file(
            &format!("{id}.manifest"),
            &format!("nil-module 1\nid {id}\nsource {id}.nil\nexport 0 0\n{extra}"),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn value(source: &str, paths: &[PathBuf]) -> Value {
    let p = compile_with_plugins(source, SourceProfile::ExprV5, paths).unwrap();
    execute_values(&p.hir, FunctionId(0), &[], Limits::default()).unwrap()
}
#[test]
fn modules_relocate_colliding_local_names_and_keep_entries_private() {
    let f = Fixture::new();
    let a = f.module(1, "1=b(a)+1\n1=a*2", "");
    let b = f.module(2, "1=b(a)+2\n1=a*3", "");
    assert_eq!(
        value("=!plugin(1,0,10)+!plugin(2,0,10)", &[a.clone(), b.clone()]),
        Value::I64(53)
    );
    let p = compile_with_plugins(
        "=!plugin(1,0,20)",
        SourceProfile::ExprV5,
        std::slice::from_ref(&a),
    )
    .unwrap();
    assert!(p.function(1).is_none());
    // Root cannot guess relocated labels to bypass exports.
    assert_eq!(
        compile_with_plugins("=b(20)", SourceProfile::ExprV5, std::slice::from_ref(&a))
            .unwrap_err()
            .code,
        "E004"
    );
    assert_eq!(
        compile_with_plugins("=!plugin(1,1,20)", SourceProfile::ExprV5, &[a])
            .unwrap_err()
            .code,
        "E024"
    );
}
#[test]
fn transitive_imports_are_explicit_and_shared_diamond_loads_once() {
    let f = Fixture::new();
    f.module(3, "1=a+1", "");
    let a = f.module(1, "1=!plugin(3,0,a)", "import 3.manifest\n");
    let b = f.module(2, "1=!plugin(3,0,a)*2", "import 3.manifest\n");
    assert_eq!(
        value("=!plugin(1,0,20)+!plugin(2,0,10)", &[a.clone(), b]),
        Value::I64(43)
    );
    assert_eq!(
        compile_with_plugins("=!plugin(3,0,20)", SourceProfile::ExprV5, &[a])
            .unwrap_err()
            .code,
        "E024"
    );
}
#[test]
fn modules_reject_cycles_missing_exports_duplicate_ids_and_bad_manifests() {
    let f = Fixture::new();
    let root = f.module(1, "=42", "import 2.manifest\n");
    f.module(2, "=42", "import 1.manifest\n");
    assert_eq!(
        compile_with_plugins("=42", SourceProfile::ExprV5, &[root])
            .unwrap_err()
            .code,
        "E024"
    );
    for extra in [
        "export 0 1\n",
        "export 1 999\n",
        "import absent\n",
        "import ../outside\n",
        "import 2.manifest\nimport 2.manifest\n",
        "effect borrow\n",
        "types record-buffer\n",
    ] {
        let p = f.module(1, "=42", extra);
        assert_eq!(
            compile_with_plugins("=42", SourceProfile::ExprV5, &[p])
                .unwrap_err()
                .code,
            "E024",
            "{extra}"
        );
    }
    let first = f.module(1, "=42", "");
    let duplicate = f.file(
        "duplicate",
        "nil-module 1\nid 1\nsource 1.nil\nexport 0 0\n",
    );
    assert_eq!(
        compile_with_plugins("=42", SourceProfile::ExprV5, &[first, duplicate])
            .unwrap_err()
            .code,
        "E024"
    );
    for manifest in [
        "nil-module 2\nid 1\nsource 1.nil\nexport 0 0",
        "nil-module 1\nid 0\nsource 1.nil\nexport 0 0",
        "nil-module 1\nid 1\nsource 1.nil",
        "nil-module 1\nid 01\nsource 1.nil\nexport 0 0",
    ] {
        let p = f.file("invalid", manifest);
        assert_eq!(
            compile_with_plugins("=42", SourceProfile::ExprV5, &[p])
                .unwrap_err()
                .code,
            "E024"
        );
    }
}
#[test]
fn module_signature_errors_and_root_nominal_registry_are_checked() {
    let f = Fixture::new();
    let p = f.module(1, "(Packet):Packet=a{value:a.value+1}", "");
    assert_eq!(
        value(
            "record Packet(value:i,text:s)\n=!plugin(1,0,Packet(41,\"x\")).value",
            std::slice::from_ref(&p)
        ),
        Value::I64(42)
    );
    for (source, code) in [
        ("record Packet(value:i,text:s)\n=!plugin(1,0)", "E006"),
        ("record Packet(value:i,text:s)\n=!plugin(1,0,42)", "E007"),
    ] {
        assert_eq!(
            compile_with_plugins(source, SourceProfile::ExprV5, std::slice::from_ref(&p))
                .unwrap_err()
                .code,
            code
        );
    }
    let p = f.module(1, "record Extra(value:i)\n=42", "");
    assert_eq!(
        compile_with_plugins("=42", SourceProfile::ExprV5, &[p])
            .unwrap_err()
            .code,
        "E024"
    );
}
#[test]
fn module_bodies_are_snapshotted_and_single_file_hir_stays_identical() {
    let f = Fixture::new();
    let path = f.module(1, "1=a+1", "");
    let p = compile_with_plugins(
        "=!plugin(1,0,41)",
        SourceProfile::ExprV5,
        std::slice::from_ref(&path),
    )
    .unwrap();
    f.file("1.nil", "1=a+100");
    assert_eq!(
        execute_values(&p.hir, FunctionId(0), &[], Limits::default()).unwrap(),
        Value::I64(42)
    );
    for source in ["=42", "(s)=#a", ":b=!equal(\"x\",\"x\")"] {
        assert_eq!(
            compile_with_plugins(source, SourceProfile::ExprV5, &[] as &[PathBuf])
                .unwrap()
                .hir,
            compile_with_profile(source, SourceProfile::ExprV5)
                .unwrap()
                .hir
        );
    }
    assert_eq!(
        compile_with_plugins("=42", SourceProfile::ExprV4, &[path])
            .unwrap_err()
            .code,
        "E024"
    );
}
#[test]
fn imported_recursion_uses_ordinary_caller_depth_and_fuel() {
    let f = Fixture::new();
    let path = f.module(1, "1=a==0?0:a(a-1)", "");
    assert_eq!(
        value("=!plugin(1,0,20)", std::slice::from_ref(&path)),
        Value::I64(0)
    );
    let p = compile_with_plugins("=!plugin(1,0,20)", SourceProfile::ExprV5, &[path]).unwrap();
    for limits in [
        Limits {
            steps: 10000,
            call_depth: 10,
        },
        Limits {
            steps: 4,
            call_depth: 256,
        },
    ] {
        assert_eq!(
            execute_values(&p.hir, FunctionId(0), &[], limits)
                .unwrap_err()
                .code,
            "E008"
        );
    }
}

#[test]
fn module_graph_depth_function_and_aggregate_source_limits_are_bounded() {
    let f = Fixture::new();
    let many = f.module(1, &"=42\n".repeat(129), "");
    assert_eq!(
        compile_with_plugins("=42", SourceProfile::ExprV5, &[many])
            .unwrap_err()
            .code,
        "E024"
    );
    let small = f.module(1, "=42", "");
    let padded = format!(
        "=42{}",
        " ".repeat(nil_compiler::parser::MAX_SOURCE_BYTES - 5)
    );
    assert_eq!(
        compile_with_plugins(&padded, SourceProfile::ExprV5, &[small])
            .unwrap_err()
            .code,
        "E024"
    );
    for id in (1..=65).rev() {
        f.module(
            id,
            "=42",
            &if id < 65 {
                format!("import {}.manifest\n", id + 1)
            } else {
                String::new()
            },
        );
    }
    assert_eq!(
        compile_with_plugins("=42", SourceProfile::ExprV5, &[f.0.join("1.manifest")])
            .unwrap_err()
            .code,
        "E024"
    );
}

#[test]
fn new_host_effects_are_allowed_in_modules_but_rejected_in_borrow_providers() {
    let f = Fixture::new();
    for body in [
        ":s=!env(\"HOME\")",
        ":u64=!random()",
        "=!size(!directory(\".\"))",
    ] {
        let module = f.module(7, body, "");
        let caller = if body.starts_with(":s") {
            ":s=!plugin(7,0)"
        } else if body.starts_with(":u64") {
            ":u64=!plugin(7,0)"
        } else {
            "=!plugin(7,0)"
        };
        let p = compile_with_plugins(caller, SourceProfile::ExprV5, &[module.clone()]).unwrap();
        assert_eq!(
            execute_values(&p.hir, FunctionId(0), &[], Limits::default())
                .unwrap_err()
                .code,
            "E018"
        );
        std::fs::write(
            &module,
            "nil-plugin 1\nid 7\nsource 7.nil\nexport 0 0\neffect borrow\n",
        )
        .unwrap();
        assert_eq!(
            compile_with_plugins(caller, SourceProfile::ExprV5, &[module])
                .unwrap_err()
                .code,
            "E024"
        );
    }
}
