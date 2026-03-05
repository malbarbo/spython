use assert_cmd::cargo;
use insta::{assert_snapshot, glob};
use std::path::Path;
use std::process::Command;

fn run_spython(path: &Path) -> (String, String) {
    let dir = path.parent().expect("path has parent");
    let filename = path.file_name().expect("path has filename");
    let output = Command::new(cargo::cargo_bin!("spython"))
        .current_dir(dir)
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
fn missing_file() {
    let output = Command::new(cargo::cargo_bin!("spython"))
        .arg("nonexistent.py")
        .output()
        .expect("failed to run spython");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot resolve 'nonexistent.py'"));
}
