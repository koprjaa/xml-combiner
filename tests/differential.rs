// Project: xml-combiner
// File: tests/differential.rs
// Description: Replay original Python observations against the Rust CLI.
// Author: Jan Alexandr Kopřiva
// License: MIT

use std::{fs, process::Command};

fn decode(hex: &str) -> Vec<u8> {
    if hex == "-" {
        return Vec::new();
    }
    assert_eq!(hex.len() % 2, 0);
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn replay_python_differential_corpus() {
    for line in include_str!("fixtures/differential.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 8);
        let [name, mode, input, python_status, python_output, rust_status, rust_output, reason] =
            fields[..]
        else {
            unreachable!()
        };
        if (python_status, python_output) != (rust_status, rust_output) {
            assert!(
                reason.starts_with("intentional:"),
                "undocumented difference: {name}"
            );
        } else {
            assert_eq!(reason, "compatible");
        }
        let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        fs::create_dir(directory.path().join("in")).unwrap();
        fs::write(directory.path().join("in/input.xml"), decode(input)).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_xml-combiner"));
        command
            .current_dir(directory.path())
            .args(["in", "-o", "result.xml"]);
        if mode != "normal" {
            command.arg(format!("--{mode}"));
        }
        let result = command.output().unwrap();
        assert_eq!(
            result.status.code(),
            Some(rust_status.parse().unwrap()),
            "{name}/{mode}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let output = directory.path().join("result.xml");
        if rust_output == "-" {
            assert!(!output.exists(), "{name}/{mode}: unexpected output");
        } else {
            assert_eq!(
                fs::read(output).unwrap(),
                decode(rust_output),
                "{name}/{mode}"
            );
        }
    }
}
