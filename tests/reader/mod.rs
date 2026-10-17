// Project: xml-combiner
// File: tests/reader/mod.rs
// Description: Verify that parser read errors remain eligible for retries.
// Author: Jan Alexandr Kopřiva
// License: MIT
use super::*;

#[test]
fn parser_preserves_io_errors_for_retry_classification() {
    struct Broken;
    impl io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "transient fixture error",
            ))
        }
    }
    impl io::BufRead for Broken {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "transient fixture error",
            ))
        }
        fn consume(&mut self, _: usize) {}
    }
    let error = parse_reader(Broken, None, None).unwrap_err();
    assert!(error.starts_with("Read error: "), "{error}");
}
