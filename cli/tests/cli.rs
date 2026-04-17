use assert_cmd::cargo;
use indoc::{formatdoc, indoc};
use insta::{assert_snapshot, glob};
use std::path::Path;
use std::process::Command;

/// Normalize Windows output: strip \r and convert backslashes to forward slashes.
fn normalize_output(s: &str) -> String {
    s.replace('\r', "").replace('\\', "/")
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
        normalize_output(&String::from_utf8_lossy(&output.stdout)),
        normalize_output(&String::from_utf8_lossy(&output.stderr)),
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
        normalize_output(&String::from_utf8_lossy(&output.stdout)),
        normalize_output(&String::from_utf8_lossy(&output.stderr)),
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
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check_inputs/"));
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
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check_inputs/"));
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

// --- Doctest validation tests ---

#[test]
fn check_doctest_type_error() {
    let (out, err, success) = run_check(&["doctest_type_error.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check_inputs/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn check_doctest_no_space() {
    let (out, err, success) = run_check(&["doctest_no_space.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check_inputs/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn check_doctest_continuation_no_space() {
    let (out, err, success) = run_check(&["doctest_continuation_no_space.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check_inputs/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn check_doctest_bare_name_allowed() {
    // A bare `>>> y` line is a value-display idiom in doctests and must not
    // trigger the BARE_EXPRESSION lint — otherwise teaching examples like
    // `>>> x: int = 2` / `>>> x` / `2` would fail level-0 checks.
    let (out, err, success) = run_check(&["doctest_bare_name.py"], &[]);
    assert!(success, "stdout={out} stderr={err}");
    assert_eq!(out, "");
    assert_eq!(
        err,
        "Running tests...\n2 tests, 2 successes, 0 failures and 0 errors.\n"
    );
}

#[test]
fn check_doctest_level_restriction() {
    // `if` inside a doctest must be rejected at level 0 like any other code.
    let (out, err, success) = run_check(&["doctest_level_restriction.py"], &["--level", "0"]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check_inputs/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn check_doctest_module_level() {
    let (out, err, success) = run_check(&["doctest_module_level.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check_inputs/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn check_doctest_cross_module() {
    // Doctest references a name imported from a sibling first-party file
    // (`from helper import square`). Type-checking the doctest must resolve
    // `square` through the import so that the wrong-type call is caught.
    let subdir = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/check_inputs/doctest_cross_module"
    ));
    let output = Command::new(cargo::cargo_bin!("spython"))
        .current_dir(subdir)
        .args(["check", "--level", "4", "main.py"])
        .output()
        .expect("failed to run spython");
    let err = normalize_output(&String::from_utf8_lossy(&output.stderr));
    assert!(
        !output.status.success(),
        "expected failure but succeeded; stderr={err}"
    );
    assert!(
        err.contains("in doctest of use")
            && err.contains("Argument to function `square` is incorrect"),
        "expected cross-module doctest error, got: {err}"
    );
}

#[test]
fn run_rejects_doctest_type_error() {
    // `spython run` should reject doctest type errors even though it never
    // executes the doctests themselves — they are part of the file and must
    // pass the same type checks as any other code.
    let path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/check_inputs/doctest_type_error.py"
    ));
    let dir = path.parent().expect("path has parent");
    let filename = path.file_name().expect("path has filename");
    let output = Command::new(cargo::cargo_bin!("spython"))
        .current_dir(dir)
        .args(["run", "--level", "4"])
        .arg(filename)
        .output()
        .expect("failed to run spython");
    assert!(!output.status.success());
    let err = normalize_output(&String::from_utf8_lossy(&output.stderr));
    assert!(
        err.contains("in doctest of add"),
        "expected doctest error in stderr, got: {err}"
    );
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
        normalize_output(&String::from_utf8_lossy(&output.stdout)),
        normalize_output(&String::from_utf8_lossy(&output.stderr)),
    )
}

#[test]
#[ignore]
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
        normalize_output(&String::from_utf8_lossy(&output.stdout)),
        normalize_output(&String::from_utf8_lossy(&output.stderr)),
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
    let null_path = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let code = formatdoc! {"
        with open('{null_path}') as f:
            pass
        print('ok')
    "};
    let (out, _, success) = run_level(5, &code);
    assert!(success);
    assert_eq!(out, "ok\n");
}

// --- Non-boolean condition tests (levels 0–3) ---

#[test]
fn level1_forbids_non_bool_if() {
    let (_, err, success) = run_level(
        1,
        indoc! {"
        x: int = 3
        if x:
            pass
    "},
    );
    assert!(!success);
    assert!(
        err.contains("non-boolean-condition"),
        "expected non-boolean-condition in stderr: {err}"
    );
}

#[test]
fn level1_allows_bool_if() {
    let (out, err, success) = run_level(
        1,
        indoc! {"
        x: int = 3
        if x > 0:
            print('ok')
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "ok\n");
}

#[test]
fn level1_forbids_non_bool_elif() {
    let (_, err, success) = run_level(
        1,
        indoc! {"
        def f(x: int) -> None:
            if x > 0:
                pass
            elif x:
                pass
    "},
    );
    assert!(!success);
    assert!(err.contains("non-boolean-condition"), "stderr: {err}");
}

#[test]
fn level3_forbids_non_bool_while() {
    let (_, err, success) = run_level(
        3,
        indoc! {"
        x: int = 3
        while x:
            x = x - 1
    "},
    );
    assert!(!success);
    assert!(err.contains("non-boolean-condition"), "stderr: {err}");
}

#[test]
fn level3_allows_bool_while() {
    let (out, _, success) = run_level(
        3,
        indoc! {"
        x: int = 3
        while x > 0:
            x -= 1
        print(x)
    "},
    );
    assert!(success);
    assert_eq!(out, "0\n");
}

#[test]
fn level1_forbids_non_bool_ternary() {
    let (_, err, success) = run_level(
        1,
        indoc! {"
        def f(x: int) -> int:
            return 1 if x else 0
    "},
    );
    assert!(!success);
    assert!(err.contains("non-boolean-condition"), "stderr: {err}");
}

#[test]
fn level1_forbids_non_bool_and_operand() {
    let (_, err, success) = run_level(
        1,
        indoc! {"
        def f(x: int, y: int) -> bool:
            return x and y > 0
    "},
    );
    assert!(!success);
    assert!(err.contains("non-boolean-condition"), "stderr: {err}");
}

#[test]
fn level1_forbids_non_bool_or_operand() {
    let (_, err, success) = run_level(
        1,
        indoc! {"
        def f(x: int, y: int) -> bool:
            return x > 0 or y
    "},
    );
    assert!(!success);
    assert!(err.contains("non-boolean-condition"), "stderr: {err}");
}

#[test]
fn level1_forbids_non_bool_not_operand() {
    let (_, err, success) = run_level(
        1,
        indoc! {"
        def f(x: int) -> bool:
            return not x
    "},
    );
    assert!(!success);
    assert!(err.contains("non-boolean-condition"), "stderr: {err}");
}

#[test]
fn level1_allows_bool_logical_ops() {
    let (out, err, success) = run_level(
        1,
        indoc! {"
        def f(x: int, y: int) -> bool:
            return (x > 0 and y > 0) or not (x == y)
        print(f(1, 2))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "True\n");
}

#[test]
fn level1_forbids_non_bool_assert() {
    let (_, err, success) = run_level(
        1,
        indoc! {"
        x: int = 1
        assert x
    "},
    );
    assert!(!success);
    assert!(err.contains("non-boolean-condition"), "stderr: {err}");
}

// --- Bool in arithmetic tests (levels 0–3) ---

#[test]
fn level0_forbids_bool_plus_int() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            return x + True
    "},
    );
    assert!(!success);
    assert!(
        err.contains("bool-in-arithmetic"),
        "expected bool-in-arithmetic in stderr: {err}"
    );
}

#[test]
fn level0_forbids_int_times_bool() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int, b: bool) -> int:
            return x * b
    "},
    );
    assert!(!success);
    assert!(err.contains("bool-in-arithmetic"), "stderr: {err}");
}

#[test]
fn level0_forbids_bool_sub() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            return True - x
    "},
    );
    assert!(!success);
    assert!(err.contains("bool-in-arithmetic"), "stderr: {err}");
}

#[test]
fn level0_forbids_bool_div() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> float:
            return x / True
    "},
    );
    assert!(!success);
    assert!(err.contains("bool-in-arithmetic"), "stderr: {err}");
}

#[test]
fn level0_forbids_bool_floordiv() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            return x // True
    "},
    );
    assert!(!success);
    assert!(err.contains("bool-in-arithmetic"), "stderr: {err}");
}

#[test]
fn level0_forbids_bool_mod() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            return x % True
    "},
    );
    assert!(!success);
    assert!(err.contains("bool-in-arithmetic"), "stderr: {err}");
}

#[test]
fn level0_forbids_bool_pow() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            return x ** True
    "},
    );
    assert!(!success);
    assert!(err.contains("bool-in-arithmetic"), "stderr: {err}");
}

#[test]
fn level3_forbids_bool_aug_assign() {
    let (_, err, success) = run_level(
        3,
        indoc! {"
        def f() -> int:
            x: int = 0
            x += True
            return x
    "},
    );
    assert!(!success);
    assert!(err.contains("bool-in-arithmetic"), "stderr: {err}");
}

#[test]
fn level0_forbids_unary_minus_bool() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(b: bool) -> int:
            return -b
    "},
    );
    assert!(!success);
    assert!(err.contains("bool-in-arithmetic"), "stderr: {err}");
}

#[test]
fn level0_forbids_unary_plus_bool() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(b: bool) -> int:
            return +b
    "},
    );
    assert!(!success);
    assert!(err.contains("bool-in-arithmetic"), "stderr: {err}");
}

#[test]
fn level0_allows_int_plus_int() {
    let (out, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int, y: int) -> int:
            return x + y
        print(f(2, 3))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "5\n");
}

#[test]
fn level0_allows_unary_minus_int() {
    let (out, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            return -x
        print(f(5))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "-5\n");
}

#[test]
fn level4_allows_bool_arithmetic() {
    let (out, err, success) = run_level(
        4,
        indoc! {"
        def f(x: int) -> int:
            return x + True
        print(f(2))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "3\n");
}

#[test]
fn level4_allows_non_bool_if() {
    let (out, err, success) = run_level(
        4,
        indoc! {"
        x: int = 3
        if x:
            print('ok')
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "ok\n");
}

// --- Chained comparison tests ---

#[test]
fn level0_forbids_chained_comparison() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(a: int, b: int, c: int) -> bool:
            return a < b < c
    "},
    );
    assert!(!success);
    assert!(
        err.contains("chained-comparison"),
        "expected chained-comparison in stderr: {err}"
    );
}

#[test]
fn level1_forbids_mixed_chained_comparison() {
    let (_, err, success) = run_level(
        1,
        indoc! {"
        def f(a: int, b: int, c: int) -> bool:
            return a == b != c
    "},
    );
    assert!(!success);
    assert!(err.contains("chained-comparison"), "stderr: {err}");
}

#[test]
fn level0_allows_single_comparison() {
    let (out, err, success) = run_level(
        0,
        indoc! {"
        def f(a: int, b: int) -> bool:
            return a < b
        print(f(1, 2))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "True\n");
}

#[test]
fn level4_allows_chained_comparison() {
    let (out, err, success) = run_level(
        4,
        indoc! {"
        def f(a: int, b: int, c: int) -> bool:
            return a < b < c
        print(f(1, 2, 3))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "True\n");
}

// --- Bare expression statement tests ---

#[test]
fn level0_forbids_bare_binop_statement() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            x + 1
            return x
    "},
    );
    assert!(!success);
    assert!(
        err.contains("bare-expression"),
        "expected bare-expression in stderr: {err}"
    );
}

#[test]
fn level0_forbids_bare_name_statement() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            x
            return x
    "},
    );
    assert!(!success);
    assert!(err.contains("bare-expression"), "stderr: {err}");
}

#[test]
fn level0_forbids_bare_compare_statement() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            x == 0
            return x
    "},
    );
    assert!(!success);
    assert!(err.contains("bare-expression"), "stderr: {err}");
}

#[test]
fn level0_allows_call_statement() {
    let (out, _, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> None:
            print(x)
        f(42)
    "},
    );
    assert!(success);
    assert_eq!(out, "42\n");
}

#[test]
fn level0_allows_docstring() {
    let (out, err, success) = run_level(
        0,
        indoc! {r#"
        def f(x: int) -> int:
            """Double x."""
            return x * 2
        print(f(3))
    "#},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "6\n");
}

#[test]
fn level4_allows_bare_expression_statement() {
    let (out, _, success) = run_level(
        4,
        indoc! {"
        def f(x: int) -> int:
            x + 1
            return x
        print(f(3))
    "},
    );
    assert!(success);
    assert_eq!(out, "3\n");
}

// --- Default argument tests ---

#[test]
fn level0_forbids_default_arg() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int = 0) -> int:
            return x
    "},
    );
    assert!(!success);
    assert!(
        err.contains("forbidden-default-arg"),
        "expected forbidden-default-arg in stderr: {err}"
    );
}

#[test]
fn level3_forbids_default_arg_kwonly() {
    let (_, err, success) = run_level(
        3,
        indoc! {"
        def f(*, x: int = 0) -> int:
            return x
    "},
    );
    assert!(!success);
    assert!(err.contains("forbidden-default-arg"), "stderr: {err}");
}

#[test]
fn level0_allows_function_without_default() {
    let (out, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            return x
        print(f(5))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "5\n");
}

#[test]
fn level4_allows_default_arg() {
    let (out, err, success) = run_level(
        4,
        indoc! {"
        def f(x: int = 7) -> int:
            return x
        print(f())
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "7\n");
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
    let stdout = normalize_output(&String::from_utf8_lossy(&output.stdout));
    // Strip the welcome banner (two lines: "Welcome to..." and "Type ctrl-d...")
    let stdout = match stdout.find("to exit.\n") {
        Some(pos) => stdout[pos + "to exit.\n".len()..].to_owned(),
        None => stdout,
    };
    let stdout = stdout.replace(">>> ", "").replace("... ", "");
    (
        stdout,
        normalize_output(&String::from_utf8_lossy(&output.stderr)),
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

#[test]
fn repl_time_command_evaluates_and_shows_elapsed() {
    let (out, _, success) = run_repl(":time 1 + 2\n");
    assert!(success);
    assert!(out.contains("3\n"), "expected expression output: {out}");
    assert!(
        out.contains("Time: "),
        "expected timing output for :time command: {out}"
    );
}

#[test]
fn repl_time_command_without_expression_shows_usage() {
    let (_, err, success) = run_repl(":time\n");
    assert!(success);
    assert!(err.contains("Usage: :time <expression>"), "stderr: {err}");
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
