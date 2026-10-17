// Project: xml-combiner
// File: src/main.rs
// Description: Command line argument parsing and process exit status.
// Author: Jan Alexandr Kopřiva
// License: MIT

use std::path::PathBuf;
use xml_combiner::{combine, log, Options, Report};

const USAGE: &str = r#"usage: xml-combiner [-h] [-o OUTPUT] [-r ROOT_ELEMENT] [-v] [--recursive]
                    [--validate-schema VALIDATE_SCHEMA] [--deduplicate]
                    [--flatten] [--max-retries MAX_RETRIES]
                    input_folder"#;
const HELP: &str = r#"usage: xml-combiner [-h] [-o OUTPUT] [-r ROOT_ELEMENT] [-v] [--recursive]
                    [--validate-schema VALIDATE_SCHEMA] [--deduplicate]
                    [--flatten] [--max-retries MAX_RETRIES]
                    input_folder

Combines multiple XML files from a directory into a single file.

positional arguments:
  input_folder          Path to directory containing XML files

options:
  -h, --help            show this help message and exit
  -o OUTPUT, --output OUTPUT
                        Path to output file (default: combined.xml)
  -r ROOT_ELEMENT, --root-element ROOT_ELEMENT
                        Root element name (default: combined)
  -v, --verbose         Enable verbose output (DEBUG level)
  --recursive           Recursively process XML files in subdirectories
  --validate-schema VALIDATE_SCHEMA
                        Path to XSD or DTD schema file for validation
  --deduplicate         Detect and skip duplicate elements based on content
  --flatten             Flatten structure: only extract direct children of
                        root (legacy behavior)
  --max-retries MAX_RETRIES
                        Maximum retry attempts for failed file processing
                        (default: 3)
"#;
fn fail(message: &str) -> ! {
    eprintln!("{USAGE}\nxml-combiner: error: {message}");
    std::process::exit(2)
}
fn main() {
    let mut options = Options::new(PathBuf::new(), PathBuf::from("combined.xml"));
    let mut positional = None;
    let mut normalized = Vec::new();
    let mut literal = false;
    for arg in std::env::args().skip(1) {
        if arg == "--" {
            literal = true;
        }
        if !literal && arg.starts_with("-v") && !arg.starts_with("--") && arg.len() > 2 {
            let mut tail = &arg[1..];
            while let Some(rest) = tail.strip_prefix('v') {
                normalized.push("-v".to_owned());
                tail = rest;
            }
            if !tail.is_empty() {
                normalized.push(format!("-{tail}"));
            }
        } else {
            normalized.push(arg);
        }
    }
    let mut args = normalized.into_iter().peekable();
    let mut remaining = false;
    while let Some(arg) = args.next() {
        if !remaining && arg == "--" {
            remaining = true;
            continue;
        }
        if !remaining && (arg == "--help" || arg == "-h") {
            print!("{HELP}");
            return;
        }
        if !remaining && arg.starts_with('-') && arg != "-" {
            let (flag, attached) = arg
                .split_once('=')
                .map(|(a, b)| (a, Some(b.to_owned())))
                .unwrap_or((&arg, None));
            let long = [
                "--help",
                "--output",
                "--root-element",
                "--verbose",
                "--recursive",
                "--validate-schema",
                "--deduplicate",
                "--flatten",
                "--max-retries",
            ];
            let matches: Vec<_> = long.iter().filter(|f| f.starts_with(flag)).collect();
            let flag = if flag.starts_with("--") && !long.contains(&flag) {
                if matches.len() == 1 {
                    *matches[0]
                } else if matches.len() > 1 {
                    fail(&format!(
                        "ambiguous option: {flag} could match {}",
                        matches
                            .iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                } else {
                    flag
                }
            } else {
                flag
            };
            if flag == "--help" {
                if let Some(value) = attached {
                    fail(&format!(
                        "argument -h/--help: ignored explicit argument '{value}'"
                    ));
                }
                print!("{HELP}");
                return;
            }
            let label = match flag {
                "-o" | "--output" => "-o/--output",
                "-r" | "--root-element" => "-r/--root-element",
                "-v" | "--verbose" => "-v/--verbose",
                _ => flag,
            };
            if [
                "-v",
                "--verbose",
                "--recursive",
                "--deduplicate",
                "--flatten",
            ]
            .contains(&flag)
            {
                if let Some(value) = attached {
                    fail(&format!(
                        "argument {label}: ignored explicit argument '{value}'"
                    ));
                }
                match flag {
                    "-v" | "--verbose" => options.verbose = true,
                    "--recursive" => options.recursive = true,
                    "--deduplicate" => options.deduplicate = true,
                    _ => options.flatten = true,
                }
                continue;
            }
            let (flag, attached) = if arg.starts_with("-o")
                && !arg.starts_with("--")
                && arg.len() > 2
                && !arg.starts_with("-o=")
            {
                ("-o", Some(arg[2..].to_owned()))
            } else if arg.starts_with("-r")
                && !arg.starts_with("--")
                && arg.len() > 2
                && !arg.starts_with("-r=")
            {
                ("-r", Some(arg[2..].to_owned()))
            } else {
                (flag, attached)
            };
            if ![
                "-o",
                "--output",
                "-r",
                "--root-element",
                "--validate-schema",
                "--max-retries",
            ]
            .contains(&flag)
            {
                fail(&format!("unrecognized arguments: {arg}"));
            }
            let label = match flag {
                "-o" | "--output" => "-o/--output",
                "-r" | "--root-element" => "-r/--root-element",
                _ => flag,
            };
            let value = attached
                .or_else(|| {
                    if args
                        .peek()
                        .is_some_and(|s| !s.starts_with('-') || s.parse::<i64>().is_ok())
                    {
                        args.next()
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| fail(&format!("argument {label}: expected one argument")));
            match flag {
                "-o" | "--output" => options.output = value.into(),
                "-r" | "--root-element" => options.root = value,
                "--validate-schema" => options.schema = Some(value.into()),
                _ => {
                    options.max_retries = value.parse().unwrap_or_else(|_| {
                        fail(&format!(
                            "argument --max-retries: invalid int value: '{value}'"
                        ))
                    })
                }
            }
        } else if positional.replace(arg.clone()).is_some() {
            fail(&format!("unrecognized arguments: {arg}"));
        }
    }
    options.input = positional
        .unwrap_or_else(|| fail("the following arguments are required: input_folder"))
        .into();
    match combine(&options, &mut Report::default()) {
        Ok(()) => {}
        Err(e) => {
            if !e.is_empty() {
                log("ERROR", &e);
            }
            std::process::exit(1);
        }
    }
}
