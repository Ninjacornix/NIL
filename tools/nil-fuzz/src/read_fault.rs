//! Acquisition faults: real native reads are interposed, reference reads use the
//! same prefix/EIO contract. Never inject a production runtime implementation.
use std::{fs, path::Path, process::Command};

pub const FAMILY_COUNT: usize = 14;
pub fn case(index: usize, seed: u64) -> (&'static str, String, Option<usize>) {
    let large = 67_108_864 - 4096 - (seed % 256) as usize;
    let read = "#!read(c)";
    let prefix = format!("=large=!bytes({large},0);");
    match index {
        0 => ("io_fault_without_pressure", format!("={read}"), Some(8192)),
        1 => (
            "io_quota_real_read",
            format!("{prefix}n={read};#large+n"),
            None,
        ),
        2 => (
            "io_fault_below_quota",
            format!("{prefix}n={read};#large+n"),
            Some(1),
        ),
        3 => (
            "io_quota_before_late_fault",
            format!("{prefix}n={read};#large+n"),
            Some(8192),
        ),
        4 => (
            "io_buffer_retention",
            format!("=large=!buffer({},0);n={read};#large+n", large / 8),
            Some(8192),
        ),
        5 => (
            "io_record_child",
            format!("=large=Cell(!bytes({large},0),0);n={read};#large.text+n"),
            Some(8192),
        ),
        6 => (
            "io_record_buffer_child",
            format!("=large=!buffer[Cell](1,Cell(!bytes({large},0),0));n={read};#large[0].text+n"),
            Some(8192),
        ),
        7 => (
            "io_map_and_child",
            format!("{prefix}m=!map();n={read};#large+!size(m)+n"),
            Some(8192),
        ),
        8 => (
            "io_selected_lazy",
            format!("{prefix}n=true?{read}:0;#large+n"),
            Some(8192),
        ),
        9 => (
            "io_callee_retention",
            format!("{prefix}b(large,c)\n(s,s)=#!read(b)+#a"),
            Some(8192),
        ),
        10 => (
            "io_unselected_lazy",
            format!("{prefix}n=false?{read}:0;#large+n"),
            Some(8192),
        ),
        11 => (
            "io_dead_value",
            format!("=dead=!bytes({large},0);{read}"),
            Some(8192),
        ),
        12 => (
            "io_short_success",
            format!("{prefix}n={read};#large+n"),
            None,
        ),
        13 => (
            "io_fault_before_prefix",
            format!("{prefix}n={read};#large+n"),
            Some(0),
        ),
        _ => unreachable!(),
    }
}
pub fn build_library(root: &Path) -> Result<std::path::PathBuf, String> {
    if !cfg!(any(target_os = "macos", target_os = "linux")) {
        return Err("read fault families require Darwin or Linux/glibc interposition".into());
    }
    let source = root.join("read-fault.c");
    fs::write(&source, include_str!("../fixtures/read_fault.c")).map_err(|e| e.to_string())?;
    let library = root.join(if cfg!(target_os = "macos") {
        "read-fault.dylib"
    } else {
        "read-fault.so"
    });
    let mut command = Command::new(std::env::var_os("NIL_CLANG").unwrap_or_else(|| "clang".into()));
    command.args([
        "-std=c11",
        "-D_GNU_SOURCE",
        "-Werror",
        "-Wno-deprecated-declarations",
    ]);
    if cfg!(target_os = "macos") {
        command.arg("-dynamiclib");
    } else {
        command.args(["-shared", "-fPIC", "-ldl"]);
    }
    let output = command
        .arg(&source)
        .arg("-o")
        .arg(&library)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "read fault fixture compilation: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(library)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acquisition_fault_matrix_retains_all_fourteen_families() {
        let sources: Vec<_> = (0..FAMILY_COUNT).map(|i| case(i, 23)).collect();
        assert_eq!(sources.len(), 14);
        assert!(sources.iter().any(|(_, _, f)| *f == Some(0)));
        assert!(sources.iter().any(|(_, _, f)| *f == Some(1)));
        assert!(sources.iter().any(|(n, _, _)| *n == "io_unselected_lazy"));
    }
}
