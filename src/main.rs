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
fn decimal_digit(c: char) -> Option<u32> {
    // Match the Unicode decimal digits accepted by the original integer parser.
    const ZEROES: &[u32] = &[
        0x30, 0x660, 0x6f0, 0x7c0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6, 0xc66, 0xce6, 0xd66,
        0xde6, 0xe50, 0xed0, 0xf20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946, 0x19d0, 0x1a80, 0x1a90,
        0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0, 0xa9f0, 0xaa50, 0xabf0,
        0xff10, 0x104a0, 0x10d30, 0x11066, 0x110f0, 0x11136, 0x111d0, 0x112f0, 0x11450, 0x114d0,
        0x11650, 0x116c0, 0x11730, 0x118e0, 0x11950, 0x11c50, 0x11d50, 0x11da0, 0x16a60, 0x16ac0,
        0x16b50, 0x1d7ce, 0x1d7d8, 0x1d7e2, 0x1d7ec, 0x1d7f6, 0x1e140, 0x1e2f0, 0x1e950, 0x1fbf0,
    ];
    ZEROES
        .iter()
        .find_map(|zero| (c as u32).checked_sub(*zero).filter(|digit| *digit < 10))
}
fn negative_number(value: &str) -> bool {
    let Some(body) = value.strip_prefix('-') else {
        return false;
    };
    let digits = |part: &str| part.chars().all(|c| decimal_digit(c).is_some());
    if let Some((integer, fraction)) = body.split_once('.') {
        digits(integer) && !fraction.is_empty() && digits(fraction)
    } else {
        !body.is_empty() && digits(body)
    }
}
fn retry_count(value: &str) -> Option<i64> {
    let value = value.trim();
    let negative = value.starts_with('-');
    let body = value.strip_prefix(['+', '-']).unwrap_or(value);
    let mut previous_digit = false;
    let mut number = 0_u64;
    for c in body.chars() {
        if c == '_' && previous_digit {
            previous_digit = false;
            continue;
        }
        let digit = decimal_digit(c)?;
        previous_digit = true;
        number = number.saturating_mul(10).saturating_add(u64::from(digit));
    }
    if !previous_digit {
        return None;
    }
    Some(if negative {
        if number > i64::MAX as u64 {
            i64::MIN
        } else {
            -(number as i64)
        }
    } else {
        number.min(i64::MAX as u64) as i64
    })
}
fn main() {
    let mut options = Options::new(PathBuf::new(), PathBuf::from("combined.xml"));
    let mut positional = None;
    let mut normalized = Vec::new();
    let mut literal = false;
    for arg in std::env::args_os().skip(1) {
        let arg = arg
            .into_string()
            .unwrap_or_else(|_| fail("arguments must be valid Unicode"));
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
                        .is_some_and(|s| s == "-" || !s.starts_with('-') || negative_number(s))
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
                "--validate-schema" => options.schema = (!value.is_empty()).then(|| value.into()),
                _ => {
                    options.max_retries = retry_count(&value).unwrap_or_else(|| {
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
    if options.input.as_os_str().is_empty() {
        options.input = ".".into();
    }
    let mut report = Report::default();
    match combine(&options, &mut report) {
        // The output is written from the files that parsed, but a skipped file is still a failed run.
        Ok(()) if report.failed_files > 0 => std::process::exit(1),
        Ok(()) => {}
        Err(e) => {
            if !e.is_empty() {
                log("ERROR", &e);
            }
            std::process::exit(1);
        }
    }
}
