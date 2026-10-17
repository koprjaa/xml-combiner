// Project: xml-combiner
// File: tests/cli.rs
// Description: Integration tests for command line switches and exit statuses.
// Author: Jan Alexandr Kopřiva
// License: MIT

use std::{
    fs,
    process::{Command, Output},
};

fn invoke(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xml-combiner"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn help_and_usage_errors() {
    let d = tempdir().unwrap();
    let r = invoke(d.path(), &["--help"]);
    assert!(r.status.success());
    assert!(String::from_utf8_lossy(&r.stdout).contains("--max-retries"));
    for args in [
        vec![],
        vec!["--unknown"],
        vec!["in", "--max-retries", "x"],
        vec!["in", "-o"],
        vec!["in", "extra"],
        vec!["--help=invalid"],
    ] {
        let r = invoke(d.path(), &args);
        assert_eq!(r.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&r.stderr).contains("error:"));
    }
}
#[test]
fn default_output_and_runtime_failures() {
    let d = tempdir().unwrap();
    fs::create_dir(d.path().join("in")).unwrap();
    assert_eq!(invoke(d.path(), &["in"]).status.code(), Some(1));
    assert_eq!(invoke(d.path(), &["missing"]).status.code(), Some(1));
    fs::write(d.path().join("in/a.xml"), "<a/>").unwrap();
    assert!(invoke(d.path(), &["in"]).status.success());
    assert_eq!(
        fs::read_to_string(d.path().join("combined.xml")).unwrap(),
        "<?xml version='1.0' encoding='utf-8'?>\n<combined><a /></combined>"
    );
    assert_eq!(
        invoke(d.path(), &["in", "-o", "../escaped.xml"])
            .status
            .code(),
        Some(1)
    );
}
#[test]
fn all_switches_work_together() {
    let d = tempdir().unwrap();
    fs::create_dir_all(d.path().join("in/deep")).unwrap();
    fs::write(d.path().join("in/a.xml"), "<r><i/></r>").unwrap();
    fs::write(d.path().join("in/deep/b.XML"), "<r><i/></r>").unwrap();
    let r = invoke(
        d.path(),
        &[
            "in",
            "-o",
            "result/output.xml",
            "-r",
            "items",
            "-v",
            "--recursive",
            "--deduplicate",
            "--flatten",
            "--validate-schema",
            "missing.xsd",
            "--max-retries",
            "5",
        ],
    );
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert_eq!(
        fs::read_to_string(d.path().join("result/output.xml")).unwrap(),
        "<?xml version='1.0' encoding='utf-8'?>\n<items><i /></items>"
    );
    let log = String::from_utf8_lossy(&r.stderr);
    assert!(log.contains("DEBUG"));
    assert!(log.contains("Schema file not found, skipping validation: missing.xsd"));
    assert!(log.contains("Successfully processed 2 of 2 files"));
}
#[test]
fn long_equals_short_attached_and_abbreviations() {
    let d = tempdir().unwrap();
    fs::create_dir(d.path().join("in")).unwrap();
    fs::write(d.path().join("in/a.xml"), "<r/>").unwrap();
    assert!(
        invoke(d.path(), &["in", "--out=x.xml", "-rnew", "--max-r=1"])
            .status
            .success()
    );
    assert!(fs::read_to_string(d.path().join("x.xml"))
        .unwrap()
        .contains("<new>"));
}

fn tempdir() -> std::io::Result<tempfile::TempDir> {
    tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR"))
}

#[cfg(unix)]
#[test]
fn relative_symlink_escape_is_rejected_and_absolute_target_is_allowed() {
    let d = tempdir().unwrap();
    let work = d.path().join("work");
    fs::create_dir(&work).unwrap();
    fs::create_dir(work.join("in")).unwrap();
    fs::write(work.join("in/a.xml"), "<r/>").unwrap();
    std::os::unix::fs::symlink(d.path(), work.join("link")).unwrap();
    let r = invoke(&work, &["in", "-o", "link/escaped.xml"]);
    assert_eq!(r.status.code(), Some(1));
    assert!(!d.path().join("escaped.xml").exists());
    let target = d.path().join("explicit.xml");
    assert!(invoke(&work, &["in", "-o", target.to_str().unwrap()])
        .status
        .success());
    assert!(target.exists());
}
#[test]
fn input_file_instead_of_directory_is_an_error() {
    let d = tempdir().unwrap();
    fs::write(d.path().join("file"), "<r/>").unwrap();
    let r = invoke(d.path(), &["file"]);
    assert_eq!(r.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&r.stderr).contains("Path is not a directory: file"));
}

#[test]
fn clustered_flags_and_short_equals_values() {
    let d = tempdir().unwrap();
    fs::create_dir(d.path().join("in")).unwrap();
    fs::write(d.path().join("in/a.xml"), "<r/>").unwrap();
    let r = invoke(d.path(), &["in", "-vvo=x.xml", "-r=items"]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(fs::read_to_string(d.path().join("x.xml"))
        .unwrap()
        .contains("<items>"));
}
#[test]
fn missing_option_value_uses_original_alias_label() {
    let d = tempdir().unwrap();
    let r = invoke(d.path(), &["in", "-o"]);
    assert_eq!(r.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&r.stderr).contains("argument -o/--output: expected one argument")
    );
}

#[cfg(unix)]
#[test]
fn dangling_output_symlinks_are_resolved_before_checking_boundaries() {
    let d = tempdir().unwrap();
    let work = d.path().join("work");
    fs::create_dir(&work).unwrap();
    fs::create_dir(work.join("in")).unwrap();
    fs::write(work.join("in/a.xml"), "<r/>").unwrap();
    let target = d.path().join("not-created.xml");
    std::os::unix::fs::symlink(&target, work.join("link.xml")).unwrap();
    assert_eq!(
        invoke(&work, &["in", "-o", "link.xml"]).status.code(),
        Some(1)
    );
    assert!(!target.exists());
    assert!(invoke(
        &work,
        &["in", "-o", work.join("link.xml").to_str().unwrap()]
    )
    .status
    .success());
    assert!(target.exists());
}

#[test]
fn option_values_accept_dash_and_negative_decimal_paths() {
    let d = tempdir().unwrap();
    fs::create_dir(d.path().join("in")).unwrap();
    fs::write(d.path().join("in/a.xml"), "<r/>").unwrap();
    for name in ["-", "-1.5", "-.5"] {
        let result = invoke(d.path(), &["in", "-o", name]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(d.path().join(name).exists());
    }
}

#[test]
fn empty_input_means_current_directory_and_empty_schema_is_disabled() {
    let d = tempdir().unwrap();
    fs::write(d.path().join("a.xml"), "<r/>").unwrap();
    let result = invoke(d.path(), &["", "--validate-schema", ""]);
    assert!(result.status.success());
    assert!(!String::from_utf8_lossy(&result.stderr).contains("Schema file not found"));
}

#[cfg(unix)]
#[test]
fn non_unicode_arguments_report_usage_without_panicking() {
    use std::os::unix::ffi::OsStringExt;
    let result = Command::new(env!("CARGO_BIN_EXE_xml-combiner"))
        .arg(std::ffi::OsString::from_vec(vec![0xff]))
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("arguments must be valid Unicode"));
}

#[test]
fn excessive_nesting_does_not_abort_other_files() {
    let d = tempdir().unwrap();
    fs::create_dir(d.path().join("in")).unwrap();
    fs::write(
        d.path().join("in/a.xml"),
        format!("{}{}", "<r>".repeat(30000), "</r>".repeat(30000)),
    )
    .unwrap();
    fs::write(d.path().join("in/b.xml"), "<ok/>").unwrap();
    for flags in [vec![], vec!["--deduplicate"], vec!["--flatten"]] {
        let mut args = vec!["in"];
        args.extend(flags);
        let result = invoke(d.path(), &args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stderr).contains("Failed to process 1 files"));
    }
}

#[cfg(unix)]
#[test]
fn recursive_discovery_skips_permission_denied_subdirectories() {
    use std::os::unix::{fs::PermissionsExt, process::CommandExt};
    let d = tempdir().unwrap();
    fs::set_permissions(d.path(), fs::Permissions::from_mode(0o777)).unwrap();
    fs::create_dir(d.path().join("in")).unwrap();
    fs::write(d.path().join("in/good.xml"), "<ok/>").unwrap();
    let locked = d.path().join("in/locked");
    fs::create_dir(&locked).unwrap();
    fs::write(locked.join("hidden.xml"), "<hidden/>").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o0)).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_xml-combiner"));
    command.current_dir(d.path()).args(["in", "--recursive"]);
    // Root bypasses permission bits, so run the fixture with an unprivileged UID.
    if unsafe { libc::geteuid() } == 0 {
        command.uid(65534).gid(65534);
    }
    let result = command.output().unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read_to_string(d.path().join("combined.xml")).unwrap(),
        "<?xml version='1.0' encoding='utf-8'?>\n<combined><ok /></combined>"
    );
}

#[test]
fn retry_counts_accept_python_decimal_syntax_and_large_values() {
    let d = tempdir().unwrap();
    fs::create_dir(d.path().join("in")).unwrap();
    fs::write(d.path().join("in/a.xml"), "<r/>").unwrap();
    for value in ["1_0", " 2 ", "١٢", "9999999999999999999999999999999"] {
        assert!(
            invoke(d.path(), &["in", "--max-retries", value])
                .status
                .success(),
            "{value}"
        );
    }
    for value in ["1__0", "_1", "1_", "+", ""] {
        assert_eq!(
            invoke(d.path(), &["in", "--max-retries", value])
                .status
                .code(),
            Some(2),
            "{value}"
        );
    }
    assert_eq!(
        invoke(
            d.path(),
            &["in", "--max-retries", "-9999999999999999999999999999999"]
        )
        .status
        .code(),
        Some(1)
    );
    assert!(invoke(d.path(), &["in", "-o", "-١.٢"]).status.success());
}

#[cfg(unix)]
#[test]
fn read_only_destination_is_not_replaced() {
    use std::os::unix::{fs::PermissionsExt, process::CommandExt};
    let d = tempdir().unwrap();
    fs::set_permissions(d.path(), fs::Permissions::from_mode(0o777)).unwrap();
    fs::create_dir(d.path().join("in")).unwrap();
    fs::write(d.path().join("in/a.xml"), "<r/>").unwrap();
    let output = d.path().join("out.xml");
    fs::write(&output, "old output").unwrap();
    fs::set_permissions(&output, fs::Permissions::from_mode(0o444)).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_xml-combiner"));
    command.current_dir(d.path()).args(["in", "-o", "out.xml"]);
    if unsafe { libc::geteuid() } == 0 {
        command.uid(65534).gid(65534);
    }
    let result = command.output().unwrap();
    fs::set_permissions(&output, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(fs::read_to_string(&output).unwrap(), "old output");
    assert_eq!(fs::read_dir(d.path()).unwrap().count(), 2);
}

#[cfg(unix)]
#[test]
fn excessive_output_symlink_chains_fail_cleanly() {
    let d = tempdir().unwrap();
    fs::create_dir(d.path().join("in")).unwrap();
    fs::write(d.path().join("in/a.xml"), "<r/>").unwrap();
    for index in 0..45 {
        std::os::unix::fs::symlink(
            format!("link{}", index + 1),
            d.path().join(format!("link{index}")),
        )
        .unwrap();
    }
    let result = invoke(d.path(), &["in", "-o", "link0"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(!d.path().join("link45").exists());
}
