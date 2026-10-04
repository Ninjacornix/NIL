use nil_compiler::{
    SourceProfile, compile_with_plugins, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new(manifest: &str, provider: &str) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "nil-plugin-test-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("provider.nil"), provider).unwrap();
        std::fs::write(dir.join("plugin"), manifest).unwrap();
        Self(dir)
    }
    fn path(&self) -> PathBuf {
        self.0.join("plugin")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
const HEADER: &str = "nil-plugin 1\nid 7\nsource provider.nil\neffect borrow\nexport 0 0\n";
#[test]
fn loading_uses_typed_records_and_provider_effects_are_derived() {
    let fixture = Fixture::new(HEADER, "(Packet):Packet=a{count:a.count+1}\n");
    let p = compile_with_plugins(
        "record Packet(count:i,text:s)\n=!plugin(7,0,Packet(41,\"live\")).count",
        SourceProfile::ExprV5,
        &[fixture.path()],
    )
    .unwrap();
    assert_eq!(
        execute_values(&p.hir, nil_hir::FunctionId(0), &[], Limits::default()).unwrap(),
        Value::I64(42)
    );
    assert_eq!(nil_hir::plugin::providers(&p.hir).len(), 1);
}
#[test]
fn forged_borrow_declarations_reject_allocation_and_host_effects() {
    for provider in [
        "(s)=!out(a)",
        ":s=!bytes(1,0)",
        "(s):s=!concat(a,a)",
        "(m):m=!put(a,\"k\",1)",
    ] {
        let f = Fixture::new(HEADER, provider);
        assert_eq!(
            compile_with_plugins("=42", SourceProfile::ExprV5, &[f.path()])
                .unwrap_err()
                .code,
            "E024"
        );
    }
}
#[test]
fn manifests_reject_unknown_versions_effects_exports_and_duplicate_ids() {
    for manifest in [
        HEADER.replace("nil-plugin 1", "nil-plugin 2"),
        HEADER.replace("effect borrow", "effect host"),
        HEADER.replace("export 0 0", "export 0 999"),
        format!("{HEADER}export 0 1\n"),
        HEADER.replace("source provider.nil", "source ../provider.nil"),
        HEADER.replace("id 7", "id 07"),
    ] {
        let f = Fixture::new(&manifest, "1=a");
        assert_eq!(
            compile_with_plugins("=42", SourceProfile::ExprV5, &[f.path()])
                .unwrap_err()
                .code,
            "E024"
        );
    }
    let f = Fixture::new(HEADER, "1=a");
    assert_eq!(
        compile_with_plugins("=42", SourceProfile::ExprV5, &[f.path(), f.path()])
            .unwrap_err()
            .code,
        "E024"
    );
}
#[test]
fn provider_recursion_nested_plugins_and_new_nominal_types_are_rejected() {
    for source in [
        "1=a==0?0:a(a-1)",
        "(s,s):b=!equal(a,b)",
        "record Extra(value:i)\n=42",
    ] {
        let f = Fixture::new(HEADER, source);
        assert_eq!(
            compile_with_plugins("=42", SourceProfile::ExprV5, &[f.path()])
                .unwrap_err()
                .code,
            "E024"
        );
    }
}
#[test]
fn unknown_calls_and_invalid_signatures_preserve_static_codes() {
    assert_eq!(
        compile_with_profile("=!plugin(7,0)", SourceProfile::ExprV5)
            .unwrap_err()
            .code,
        "E024"
    );
    let f = Fixture::new(HEADER, "1=a");
    for (source, code) in [
        ("=!plugin(7,1,42)", "E024"),
        ("=!plugin(7,0)", "E006"),
        ("=!plugin(7,0,true)", "E007"),
        ("=!plugin(07,0,42)", "E001"),
    ] {
        assert_eq!(
            compile_with_plugins(source, SourceProfile::ExprV5, &[f.path()])
                .unwrap_err()
                .code,
            code
        );
    }
}
#[test]
fn equality_provider_does_not_spend_caller_internal_fuel_or_call_depth() {
    let p = compile_with_profile(
        ":b=!equal(!bytes(10000,9),!bytes(10000,9))",
        SourceProfile::ExprV5,
    )
    .unwrap();
    // Two constants per constructor, two constructors, equality and return: eight steps.
    assert_eq!(
        execute_values(
            &p.hir,
            nil_hir::FunctionId(0),
            &[],
            Limits {
                steps: 8,
                call_depth: 1
            }
        )
        .unwrap(),
        Value::Bool(true)
    );
}
#[test]
fn providers_are_snapshotted_and_earlier_profiles_reject_loading() {
    let f = Fixture::new(HEADER, "1=a+1");
    let p = compile_with_plugins("=!plugin(7,0,41)", SourceProfile::ExprV5, &[f.path()]).unwrap();
    std::fs::write(f.0.join("provider.nil"), "1=a+100").unwrap();
    assert_eq!(
        execute_values(&p.hir, nil_hir::FunctionId(0), &[], Limits::default()).unwrap(),
        Value::I64(42)
    );
    assert_eq!(
        compile_with_plugins("=42", SourceProfile::ExprV4, &[f.path()])
            .unwrap_err()
            .code,
        "E024"
    );
}

#[test]
fn shipped_equality_is_available_through_the_generic_plugin_namespace() {
    for source in [
        ":b=!plugin(0,0,\"x\",\"x\")",
        ":b=!plugin(0,0,!buffer(1,-7),!buffer(1,-7))",
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        assert_eq!(
            execute_values(&p.hir, nil_hir::FunctionId(0), &[], Limits::default()).unwrap(),
            Value::Bool(true)
        );
        let providers = nil_hir::plugin::providers(&p.hir);
        assert_eq!((providers[0].id, providers[0].operation), (0, 0));
    }
}
