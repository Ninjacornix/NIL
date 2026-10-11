use nil_compiler::{
    SourceProfile,
    application::{Host, SeededHost, read_prefix},
    compile_with_profile,
    evaluator::{Limits, execute_values_with_host},
};
use nil_hir::{Diagnostic, FunctionId};
use std::io::{self, Read};

struct FailingReader {
    offset: usize,
    fault: usize,
    requested: Vec<usize>,
}
impl Read for FailingReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.requested.push(buffer.len());
        if self.offset >= self.fault {
            return Err(io::Error::other("injected EIO"));
        }
        let n = buffer.len().min(self.fault - self.offset);
        buffer[..n].fill(b'x');
        self.offset += n;
        Ok(n)
    }
}
#[test]
fn oversized_prefix_stops_before_a_later_io_failure() {
    for limit in [0, 1, 225, 8192] {
        let mut reader = FailingReader {
            offset: 0,
            fault: limit + 2,
            requested: vec![],
        };
        assert_eq!(read_prefix(&mut reader, limit).unwrap_err().code, "E013");
        assert_eq!(reader.offset, limit + 1);
    }
}
#[test]
fn errors_before_or_at_the_admissible_prefix_remain_io_failures() {
    for fault in [0, 1, 225] {
        let mut reader = FailingReader {
            offset: 0,
            fault,
            requested: vec![],
        };
        assert_eq!(read_prefix(&mut reader, 225).unwrap_err().code, "E015");
        assert_eq!(reader.offset, fault);
    }
}
#[test]
fn exact_and_short_successful_prefixes_are_admitted() {
    for size in [0, 1, 225] {
        assert_eq!(
            read_prefix(io::Cursor::new(vec![b'x'; size]), 225).unwrap(),
            vec![b'x'; size]
        );
    }
}

struct StreamingHost {
    reader: FailingReader,
    limit: Option<usize>,
}
impl Host for StreamingHost {
    fn read_with_limit(&mut self, _: &[u8], limit: usize) -> Result<Vec<u8>, Diagnostic> {
        self.limit = Some(limit);
        read_prefix(&mut self.reader, limit)
    }
}
#[test]
fn evaluator_supplies_remaining_budget_and_wrapper_preserves_read_span() {
    let source = "=large=!bytes(67108500,0);n=#!read(\"nil-quota-fault.bin\");#large+n";
    let hir = compile_with_profile(source, SourceProfile::ExprV5)
        .unwrap()
        .hir;
    let mut host = SeededHost::new(
        StreamingHost {
            reader: FailingReader {
                offset: 0,
                fault: 4096,
                requested: vec![],
            },
            limit: None,
        },
        7,
    );
    let error = execute_values_with_host(
        &hir,
        FunctionId(0),
        &[],
        Limits {
            memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
            ..Limits::default()
        },
        &mut host,
    )
    .unwrap_err();
    assert_eq!(error.code, "E013");
    assert!(error.span.is_some());
    assert_eq!(host.inner.limit, Some(225));
    assert_eq!(host.inner.reader.offset, 226);
}
#[test]
fn bounded_read_does_not_grant_host_permission() {
    let mut host = nil_compiler::application::DeniedHost;
    assert_eq!(host.read_with_limit(b"file", 0).unwrap_err().code, "E018");
}

#[test]
fn direct_file_host_read_uses_the_new_default_instead_of_the_historical_quota() {
    use nil_compiler::application::FileHost;
    let path = std::env::temp_dir().join(format!("nil-direct-host-{}.bin", std::process::id()));
    let size = nil_hir::MAX_DYNAMIC_BYTES as u64 + 1;
    std::fs::File::create(&path).unwrap().set_len(size).unwrap();
    let bytes = FileHost::default()
        .read(path.to_str().unwrap().as_bytes())
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(bytes.len() as u64, size);
    assert!(bytes.iter().all(|b| *b == 0));
}
