use assert_cmd::cargo;
use indoc::indoc;
use insta::{assert_snapshot, glob};
use std::path::Path;
use std::process::Command;

/// Normalize Windows backslashes to forward slashes in test output.
fn normalize_paths(s: &str) -> String {
    s.replace('\\', "/")
}

fn run_check(files: &[&str], extra_args: &[&str]) -> (String, String, bool) {
    let check_inputs = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check_inputs"));
    let mut cmd = Command::new(cargo::cargo_bin!("spython"));
    cmd.current_dir(check_inputs)
        .args(["check", "--level", "4"]);
    for arg in extra_args {
        cmd.arg(arg);
    }
    for file in files {
        cmd.arg(file);
    }
    let output = cmd.output().expect("failed to run spython");
    (
        normalize_paths(&String::from_utf8_lossy(&output.stdout)),
        normalize_paths(&String::from_utf8_lossy(&output.stderr)),
        output.status.success(),
    )
}

fn run_spython(path: &Path) -> (String, String) {
    let dir = path.parent().expect("path has parent");
    let filename = path.file_name().expect("path has filename");
    let output = Command::new(cargo::cargo_bin!("spython"))
        .current_dir(dir)
        .args(["run", "--level", "4"])
        .arg(filename)
        .output()
        .expect("failed to run spython");
    (
        normalize_paths(&String::from_utf8_lossy(&output.stdout)),
        normalize_paths(&String::from_utf8_lossy(&output.stderr)),
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
    assert_eq!(
        err,
        "Running tests...\n2 tests, 2 successes, 0 failures and 0 errors.\n"
    );
}

#[test]
fn check_fail() {
    let (out, err, success) = run_check(&["fail.py"], &[]);
    assert!(!success);
    let filter = normalize_paths(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check_inputs/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
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
    let filter = normalize_paths(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check_inputs/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
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

// --- Image tests ---

fn run_image(path: &Path) -> (String, String) {
    let dir = path.parent().expect("path has parent");
    let filename = path.file_name().expect("path has filename");
    let output = Command::new(cargo::cargo_bin!("spython"))
        .current_dir(dir)
        .args(["run", "--level", "5"])
        .arg(filename)
        .output()
        .expect("failed to run spython");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn run_images() {
    glob!("images/*.py", |path| {
        let (out, _err) = run_image(path);
        assert_snapshot!(format!("{out}"));
    });
}

// --- Level restriction tests ---

fn run_level(level: u8, code: &str) -> (String, String, bool) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir();
    let file = dir.join(format!("_spython_level_test_{n}.py"));
    std::fs::write(&file, code).unwrap();
    let output = Command::new(cargo::cargo_bin!("spython"))
        .current_dir(&dir)
        .args(["run", "--level", &level.to_string()])
        .arg(&file)
        .output()
        .expect("failed to run spython");
    let _ = std::fs::remove_file(&file);
    (
        normalize_paths(&String::from_utf8_lossy(&output.stdout)),
        normalize_paths(&String::from_utf8_lossy(&output.stderr)),
        output.status.success(),
    )
}

#[test]
fn level0_allows_functions() {
    let (out, _, success) = run_level(
        0,
        indoc! {"
        def double(x: int) -> int:
            return x * 2
        print(double(5))
    "},
    );
    assert!(success);
    assert_eq!(out, "10\n");
}

#[test]
fn level0_forbids_if() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            if x > 0:
                return x
            return 0
    "},
    );
    assert!(!success);
    assert!(err.contains("forbidden-selection"));
}

#[test]
fn level1_allows_if_and_functions() {
    let (out, _, success) = run_level(
        1,
        indoc! {"
        def maximo(a: int, b: int) -> int:
            if a > b:
                return a
            return b
        print(maximo(3, 5))
    "},
    );
    assert!(success);
    assert_eq!(out, "5\n");
}

#[test]
fn level1_forbids_for() {
    let (_, err, success) = run_level(
        1,
        indoc! {"
        for i in range(10):
            pass
    "},
    );
    assert!(!success);
    assert!(err.contains("forbidden-loop"));
}

#[test]
fn level1_forbids_while() {
    let (_, err, success) = run_level(
        1,
        indoc! {"
        x: int = 0
        while x < 10:
            x = x + 1
    "},
    );
    assert!(!success);
    assert!(err.contains("forbidden-loop"));
}

#[test]
fn level1_forbids_list_literal() {
    let (_, err, success) = run_level(1, "x: list[int] = [1, 2, 3]\n");
    assert!(!success);
    assert!(err.contains("forbidden-collection-literal"));
}

#[test]
fn level1_forbids_class() {
    let (_, err, success) = run_level(
        1,
        indoc! {"
        class Foo:
            x: int = 0
    "},
    );
    assert!(!success);
    assert!(err.contains("forbidden-class"));
}

#[test]
fn level1_allows_string_indexing() {
    let (out, _, success) = run_level(
        1,
        indoc! {"
        def first(s: str) -> str:
            return s[0]
        print(first('hello'))
    "},
    );
    assert!(success);
    assert_eq!(out, "h\n");
}

#[test]
fn level2_allows_class() {
    let (_, _, success) = run_level(
        2,
        indoc! {"
        from dataclasses import dataclass
        @dataclass
        class Point:
            x: int
            y: int
    "},
    );
    assert!(success);
}

#[test]
fn level2_forbids_methods() {
    let (_, err, success) = run_level(
        2,
        indoc! {"
        class Foo:
            def bar(self) -> None:
                pass
    "},
    );
    assert!(!success);
    assert!(err.contains("forbidden-class-method"));
}

#[test]
fn level2_forbids_for() {
    let (_, err, success) = run_level(
        2,
        indoc! {"
        for i in range(10):
            pass
    "},
    );
    assert!(!success);
    assert!(err.contains("forbidden-loop"));
}

#[test]
fn level2_allows_match() {
    let (out, _, success) = run_level(
        2,
        indoc! {"
        def f(x: int) -> str:
            match x:
                case 1:
                    return 'one'
                case _:
                    return 'other'
        print(f(1))
    "},
    );
    assert!(success);
    assert_eq!(out, "one\n");
}

#[test]
fn level3_allows_for_and_list() {
    let (out, _, success) = run_level(
        3,
        indoc! {"
        xs: list[int] = [1, 2, 3]
        for x in xs:
            print(x)
    "},
    );
    assert!(success);
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn level3_allows_while() {
    let (out, _, success) = run_level(
        3,
        indoc! {"
        x: int = 0
        while x < 3:
            x += 1
        print(x)
    "},
    );
    assert!(success);
    assert_eq!(out, "3\n");
}

#[test]
fn level3_forbids_comprehension() {
    let (_, err, success) = run_level(3, "x: list[int] = [i for i in range(10)]\n");
    assert!(!success);
    assert!(err.contains("forbidden-comprehension"));
}

#[test]
fn level3_forbids_lambda() {
    let (_, err, success) = run_level(3, "f = lambda x: x + 1\n");
    assert!(!success);
    assert!(err.contains("forbidden-lambda"));
}

#[test]
fn level3_forbids_dict_literal() {
    let (_, err, success) = run_level(3, "x: dict[str, int] = {'a': 1}\n");
    assert!(!success);
    assert!(err.contains("forbidden-collection-literal"));
}

#[test]
fn level4_allows_methods() {
    let (out, _, success) = run_level(
        4,
        indoc! {"
        class Counter:
            def __init__(self) -> None:
                self.n: int = 0
            def inc(self) -> None:
                self.n += 1
        c: Counter = Counter()
        c.inc()
        print(c.n)
    "},
    );
    assert!(success);
    assert_eq!(out, "1\n");
}

#[test]
fn level4_allows_comprehension() {
    let (out, _, success) = run_level(4, "print([i * 2 for i in range(3)])\n");
    assert!(success);
    assert_eq!(out, "[0, 2, 4]\n");
}

#[test]
fn level4_allows_lambda() {
    let (out, _, success) = run_level(
        4,
        indoc! {"
        f = lambda x: x + 1
        print(f(2))
    "},
    );
    assert!(success);
    assert_eq!(out, "3\n");
}

#[test]
fn level4_forbids_global() {
    let (_, err, success) = run_level(
        4,
        indoc! {"
        x: int = 0
        def f() -> None:
            global x
            x = 1
    "},
    );
    assert!(!success);
    assert!(err.contains("forbidden-construct"));
}

#[test]
fn level5_allows_global() {
    let (_, _, success) = run_level(
        5,
        indoc! {"
        x: int = 0
        def f() -> None:
            global x
            x = 1
        f()
    "},
    );
    assert!(success);
}

#[test]
fn level5_allows_try() {
    let (out, _, success) = run_level(
        5,
        indoc! {"
        try:
            x: int = 1
        except Exception:
            x = 0
        print(x)
    "},
    );
    assert!(success);
    assert_eq!(out, "1\n");
}

#[test]
fn level5_allows_with() {
    let (out, _, success) = run_level(
        5,
        indoc! {"
        with open('/dev/null') as f:
            pass
        print('ok')
    "},
    );
    assert!(success);
    assert_eq!(out, "ok\n");
}

#[test]
fn level5_allows_intenum_without_annotations() {
    let (_, _err, success) = run_level(
        5,
        indoc! {"
        from enum import IntEnum
        class Color(IntEnum):
            RED = 1
            BLUE = 2
    "},
    );
    assert!(success);
}

#[test]
fn level5_allows_strenum_without_annotations() {
    let (_, _err, success) = run_level(
        5,
        indoc! {"
        from enum import StrEnum
        class Dir(StrEnum):
            UP = 'up'
            DOWN = 'down'
    "},
    );
    assert!(success);
}

// --- REPL tests ---

fn run_repl(input: &str) -> (String, String, bool) {
    run_repl_level(input, 0)
}

fn run_repl_level(input: &str, level: u8) -> (String, String, bool) {
    let output = Command::new(cargo::cargo_bin!("spython"))
        .args(["repl", "--level", &level.to_string()])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .expect("failed to run spython repl");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    // Strip the welcome banner (two lines: "Welcome to..." and "Type ctrl-d...")
    let stdout = match stdout.find("to exit.\n") {
        Some(pos) => stdout[pos + "to exit.\n".len()..].to_owned(),
        None => stdout,
    };
    (
        stdout,
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.success(),
    )
}

#[test]
fn repl_expression() {
    let (out, _, success) = run_repl("1 + 2\n");
    assert!(success);
    assert_eq!(out, "3\n");
}

#[test]
fn repl_print() {
    let (out, _, success) = run_repl("print('hello')\n");
    assert!(success);
    assert_eq!(out, "hello\n");
}

#[test]
fn repl_multiline_function() {
    let (out, _, success) = run_repl("def f(x: int) -> int:\n    return x * 2\n\nf(3)\n");
    assert!(success);
    assert_eq!(out, "6\n");
}

#[test]
fn repl_syntax_error() {
    let (_, err, success) = run_repl("def\n");
    assert!(success); // REPL doesn't exit on errors
    assert!(
        err.contains("invalid-syntax"),
        "expected invalid-syntax in stderr: {err}"
    );
}

#[test]
fn repl_name_error() {
    let (_, err, success) = run_repl("undefined_var\n");
    assert!(success);
    assert!(
        err.contains("unresolved-reference"),
        "expected unresolved-reference in stderr: {err}"
    );
}

#[test]
fn repl_multiple_statements() {
    let (out, _, success) = run_repl("x = 10\nx * 3\n");
    assert!(success);
    assert_eq!(out, "30\n");
}

// --- REPL type checking tests ---

#[test]
fn repl_missing_annotation_is_error() {
    let (_, err, success) = run_repl("def f(x): return x\n");
    assert!(success); // REPL doesn't exit on errors
    assert!(
        err.contains("missing-param-annotation") || err.contains("missing-return-annotation"),
        "expected annotation error in stderr: {err}"
    );
}

#[test]
fn repl_annotated_function_ok() {
    let (out, err, success) = run_repl("def f(x: int) -> int:\n    return x * 2\n\nf(3)\n");
    assert!(success);
    assert!(!err.contains("missing"), "unexpected error: {err}");
    assert_eq!(out, "6\n");
}

#[test]
fn repl_level0_forbids_if() {
    let (_, err, success) = run_repl_level("if True:\n    pass\n\n", 0);
    assert!(success);
    assert!(
        err.contains("forbidden-selection"),
        "expected forbidden-selection in stderr: {err}"
    );
}

#[test]
fn repl_level1_allows_if() {
    let (out, _, success) = run_repl_level("if True:\n    print('ok')\n\n", 1);
    assert!(success);
    assert_eq!(out, "ok\n");
}

// --- Panic handler test ---

#[test]
fn panic_handler_shows_message() {
    let output = Command::new(cargo::cargo_bin!("spython"))
        .arg("--test-panic")
        .output()
        .expect("failed to run spython");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("spython: internal error"),
        "expected 'spython: internal error' in stderr: {stderr}"
    );
    assert!(
        stderr.contains("test panic"),
        "expected 'test panic' in stderr: {stderr}"
    );
    assert!(
        stderr.contains("https://github.com/malbarbo/spython/issues"),
        "expected issue URL in stderr: {stderr}"
    );
}

// --- Other tests ---

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
