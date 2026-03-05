use assert_cmd::cargo;
use insta::{assert_snapshot, glob};
use std::path::Path;
use std::process::Command;

fn run_check(files: &[&str], extra_args: &[&str]) -> (String, String, bool) {
    let check_inputs = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check_inputs"));
    let mut cmd = Command::new(cargo::cargo_bin!("spython"));
    cmd.current_dir(check_inputs).arg("check");
    for arg in extra_args {
        cmd.arg(arg);
    }
    for file in files {
        cmd.arg(file);
    }
    let output = cmd.output().expect("failed to run spython");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.success(),
    )
}

fn run_spython(path: &Path) -> (String, String) {
    let dir = path.parent().expect("path has parent");
    let filename = path.file_name().expect("path has filename");
    let output = Command::new(cargo::cargo_bin!("spython"))
        .current_dir(dir)
        .arg("run")
        .arg(filename)
        .output()
        .expect("failed to run spython");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn run_files() {
    glob!("inputs/*.py", |path| {
        let (out, err) = run_spython(path);
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn ok_simple() {
    let path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/inputs/ok_simple.py"
    ));
    let (out, err) = run_spython(path);
    assert_eq!(err, "");
    assert_eq!(out, "3\n");
}

#[test]
fn check_ok() {
    let (out, err, success) = run_check(&["ok.py"], &[]);
    assert!(success);
    assert_eq!(out, "");
    assert_eq!(err, "");
}

#[test]
fn check_fail() {
    let (out, err, success) = run_check(&["fail.py"], &[]);
    assert!(!success);
    insta::with_settings!({
        filters => vec![(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check_inputs/"), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn check_no_doctests() {
    let (out, err, success) = run_check(&["no_tests.py"], &[]);
    assert!(success);
    assert_eq!(out, "");
    assert_eq!(err, "");
}

#[test]
fn check_nonexistent_ignored() {
    let (out, err, success) = run_check(&["nonexistent.py"], &[]);
    assert!(success);
    assert_eq!(out, "");
    assert_eq!(err, "");
}

#[test]
fn check_multiple_prints_names() {
    let (out, err, success) = run_check(&["ok.py", "fail.py"], &[]);
    assert!(!success);
    insta::with_settings!({
        filters => vec![(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check_inputs/"), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn check_verbose() {
    let (out, err, success) = run_check(&["ok.py"], &["--verbose"]);
    assert!(success);
    assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
}

#[test]
fn missing_file() {
    let output = Command::new(cargo::cargo_bin!("spython"))
        .args(["run", "nonexistent.py"])
        .output()
        .expect("failed to run spython");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot resolve 'nonexistent.py'"));
}
