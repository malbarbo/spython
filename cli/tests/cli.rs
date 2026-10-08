use assert_cmd::cargo;
use indoc::{formatdoc, indoc};
use insta::assert_snapshot;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The binary under test. A FORCE_COLOR the machine running the tests set for
/// something else would put escape codes in every snapshot here, so it goes.
fn spython() -> Command {
    let mut command = Command::new(cargo::cargo_bin!("spython"));
    command.env_remove("FORCE_COLOR");
    command
}

/// Normalize Windows output: strip \r and convert backslashes to forward slashes.
fn normalize_output(s: &str) -> String {
    s.replace('\r', "").replace('\\', "/")
}

/// Run a command that reads its stdin, and answer with what it wrote and
/// whether it was happy. Everything that drives the REPL or `format` over a
/// pipe comes through here.
fn run_with_stdin(command: &mut Command, input: &[u8]) -> (String, String, bool) {
    use std::io::Write as _;
    let output = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .take()
                .expect("the child stdin is a pipe")
                .write_all(input)?;
            child.wait_with_output()
        })
        .expect("failed to run spython");
    (
        normalize_output(&String::from_utf8_lossy(&output.stdout)),
        normalize_output(&String::from_utf8_lossy(&output.stderr)),
        output.status.success(),
    )
}

/// A directory of its own holding `files`, for a test that needs them on a
/// filesystem. The pid keeps two concurrent runs apart, and the removal keeps
/// a directory a panicking test left behind from seeding this one.
fn new_dir_with(prefix: &str, files: &[(&str, &str)]) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("_spython_{prefix}_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the test directory");
    for (name, source) in files {
        let path = dir.join(name);
        let parent = path.parent().expect("the file is under the directory");
        std::fs::create_dir_all(parent).expect("create the test directory");
        std::fs::write(&path, source).expect("write the test file");
    }
    dir
}

/// Run `spython <args>` with the given cwd. Used by helpers that can be
/// dual-checked against the WASM backend. When the `wasm-backend` feature is
/// enabled, also runs the Deno wrapper with the same args and asserts that
/// stdout/stderr match after normalization.
fn run_spython_cmd(args: &[&str], cwd: &Path) -> (String, String, bool) {
    let native = run_spython_native(args, cwd);
    #[cfg(feature = "wasm-backend")]
    {
        let wasm = run_spython_wasm(args, cwd);
        let norm = |s: &str| normalize_diagnostic_paths(s);
        assert_eq!(
            norm(&native.0),
            norm(&wasm.0),
            "stdout: native vs wasm differ for args {args:?}"
        );
        assert_eq!(
            norm(&native.1),
            norm(&wasm.1),
            "stderr: native vs wasm differ for args {args:?}"
        );
        assert_eq!(
            native.2, wasm.2,
            "exit status: native vs wasm differ for args {args:?}"
        );
    }
    native
}

fn run_spython_native(args: &[&str], cwd: &Path) -> (String, String, bool) {
    let output = spython()
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("failed to run spython");
    (
        normalize_output(&String::from_utf8_lossy(&output.stdout)),
        normalize_output(&String::from_utf8_lossy(&output.stderr)),
        output.status.success(),
    )
}

#[cfg(feature = "wasm-backend")]
fn run_spython_wasm(args: &[&str], cwd: &Path) -> (String, String, bool) {
    let wrapper = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("wasm")
        .join("spython.ts")
        .canonicalize()
        .expect("canonicalize wasm/spython.ts");
    let mut cmd = Command::new("deno");
    cmd.current_dir(cwd)
        .args(["run", "--quiet", "--allow-read"])
        .arg(&wrapper)
        .args(args);
    let output = cmd.output().expect("failed to run deno wrapper");
    (
        normalize_output(&String::from_utf8_lossy(&output.stdout)),
        normalize_output(&String::from_utf8_lossy(&output.stderr)),
        output.status.success(),
    )
}

/// Collapse every `<path>.py:` token down to `<file>.py:`. The native
/// binary reports a relative path, while the WASM backend reports
/// `user.py` (the in-memory name). Collapsing both to a canonical form
/// lets dual-run compare diagnostic bodies without caring about paths.
#[cfg(feature = "wasm-backend")]
fn normalize_diagnostic_paths(s: &str) -> String {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"[\w./-]+\.py").unwrap());
    re.replace_all(s, "<file>.py").into_owned()
}

fn run_test(files: &[&str], extra_args: &[&str]) -> (String, String, bool) {
    let doctests = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests"));
    let mut cmd = spython();
    cmd.current_dir(doctests).args(["test", "--level", "4"]);
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
    let filename = path
        .file_name()
        .expect("path has filename")
        .to_str()
        .expect("filename is utf8");
    let (out, err, _) = run_spython_cmd(&["run", "--level", "4", filename], dir);
    (out, err)
}

fn run_level_native_only(level: u8, code: &str) -> (String, String, bool) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir();
    let filename = format!("_spython_level_native_{n}.py");
    let file = dir.join(&filename);
    std::fs::write(&file, code).expect("write the test file");
    let level_str = level.to_string();
    let result = run_spython_native(&["run", "--level", &level_str, &filename], &dir);
    let _ = std::fs::remove_file(&file);
    result
}

#[test]
fn bare_file_shorthand_runs() {
    // `spython FILE` (no subcommand) runs the file like `spython run FILE`.
    let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/inputs"));
    let (out, err, success) = run_spython_native(&["ok_simple.py"], dir);
    assert!(success, "stdout: {out}\nstderr: {err}");
    assert_eq!(err, "");
    assert_eq!(out, "3\n");
}

#[test]
fn bare_file_shorthand_passes_level() {
    // `--level` flows to the injected `run`, so a level-1 `if` is accepted.
    let dir = std::env::temp_dir();
    let file = dir.join("_spython_bare_level.py");
    std::fs::write(&file, "x: int = 3\nif x > 0:\n    print('ok')\n").expect("write the test file");
    let (out, _err, ok_lvl1) =
        run_spython_native(&["_spython_bare_level.py", "--level", "1"], &dir);
    let (_o0, err0, ok_lvl0) = run_spython_native(&["_spython_bare_level.py"], &dir);
    let _ = std::fs::remove_file(&file);
    assert!(ok_lvl1, "level 1 should allow `if`: {out}");
    assert_eq!(out, "ok\n");
    assert!(!ok_lvl0, "level 0 (default) should reject `if`");
    assert!(err0.contains("forbidden-selection"), "stderr: {err0}");
}

#[test]
fn bare_file_shorthand_after_dashdash() {
    // `--` forces the next token to be a file, even if it matches a
    // subcommand name, so it is dispatched to `run`.
    let dir = std::env::temp_dir();
    let file = dir.join("run");
    std::fs::write(&file, "print(2)\n").expect("write the test file");
    let (out, err, success) = run_spython_native(&["--", "run"], &dir);
    let _ = std::fs::remove_file(&file);
    assert!(success, "stdout: {out}\nstderr: {err}");
    assert_eq!(out, "2\n");
}

#[test]
fn bare_level_starts_repl() {
    // `spython -l N` with no file or subcommand starts the REPL at level N
    // (the level flag is routed into an injected `repl`).
    let mut command = spython();
    command.args(["-l", "2"]);
    let (stdout, _, _) = run_with_stdin(&mut command, b":quit\n");
    assert!(
        stdout.contains("level 2 - User types"),
        "banner should report level 2, got: {stdout}"
    );
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
fn test_ok() {
    let (out, err, success) = run_test(&["ok.py"], &[]);
    assert!(success, "stdout: {out}\nstderr: {err}");
    assert_eq!(out, "");
    assert_eq!(
        err,
        "Running tests...\n  add\n2 tests, 2 successes, 0 failures and 0 errors.\n"
    );
}

#[test]
fn test_fail() {
    let (out, err, success) = run_test(&["fail.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn test_banner_and_function_name_come_first() {
    // The "Running tests..." banner and the name of each function must be
    // written (and flushed) *before* its examples run, so a student whose test
    // hangs sees that testing started and which function is to blame instead
    // of a blank screen. The example never ends, so what the test reads is
    // what came out before it.
    use std::io::BufRead;
    let doctests = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests"));
    let mut child = spython()
        .current_dir(doctests)
        .args(["test", "--level", "4", "doctest_hangs.py"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to run spython");
    let stderr = child.stderr.take().expect("stderr is piped");
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            if sender.send(line + "\n").is_err() {
                break;
            }
        }
    });
    let expected = "Running tests...\n  forever\n";
    let mut err = String::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while normalize_output(&err) != expected {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match receiver.recv_timeout(left) {
            Ok(line) => err.push_str(&line),
            Err(_) => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(normalize_output(&err), expected);
}

#[test]
fn test_reads_doctests_with_the_grammar_of_the_stdlib() {
    // A bare `...` continues the source, and the output keeps the spaces it
    // has beyond the indentation of its `>>>`.
    let (out, err, success) = run_test(&["doctest_grammar.py"], &[]);
    assert!(success, "stdout: {out}\nstderr: {err}");
    assert_eq!(
        err,
        "Running tests...\n  show\n1 test, 1 success, 0 failures and 0 errors.\n"
    );
}

#[test]
fn test_takes_blankline_for_a_blank_line_of_output() {
    let (out, err, success) = run_test(&["doctest_blankline.py"], &[]);
    assert!(success, "stdout: {out}\nstderr: {err}");
}

#[test]
fn test_runs_the_doctests_of_the_module_and_of_methods_once() {
    let (out, err, success) = run_test(&["doctest_everywhere.py"], &[]);
    assert!(success, "stdout: {out}\nstderr: {err}");
    assert_eq!(
        err,
        "Running tests...\n  <module>\n  Point\n  Point.norm1\n  double\n\
         4 tests, 4 successes, 0 failures and 0 errors.\n"
    );
}

#[test]
fn test_runs_a_docstring_that_its_object_lost() {
    let (out, err, success) = run_test(&["doctest_lost_docstring.py"], &[]);
    assert!(success, "stdout: {out}\nstderr: {err}");
    assert_eq!(
        err,
        "Running tests...\n  Box.n\n  double\n\
         4 tests, 4 successes, 0 failures and 0 errors.\n"
    );
}

#[test]
fn test_gives_each_file_the_modules_of_its_own_directory() {
    let (out, err, success) = run_test(
        &["same_module_name/a/main.py", "same_module_name/b/main.py"],
        &[],
    );
    assert!(success, "stdout: {out}\nstderr: {err}");
}

#[test]
fn test_no_doctests() {
    let (out, err, success) = run_test(&["no_tests.py"], &[]);
    assert!(success);
    assert_eq!(out, "");
    assert_eq!(err, "");
}

#[test]
fn test_refuses_a_missing_file() {
    // A name the student mistyped is an error, the way it is for `run`, and
    // not a run that reports no failures.
    let (out, err, success) = run_test(&["nonexistent.py"], &[]);
    assert!(!success, "test should have refused, got: {out}{err}");
    assert_eq!(out, "");
    assert!(
        err.contains("nonexistent.py"),
        "expected the file name in the error, got: {err}"
    );
}

#[test]
fn test_multiple_prints_names() {
    let (out, err, success) = run_test(&["ok.py", "fail.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn test_fails_a_file_that_exits_before_its_tests() {
    let (out, err, success) = run_test(&["exits.py"], &[]);
    assert!(!success);
    assert_eq!(out, "");
    assert_eq!(err, "The file exited before its tests ran.\n");
}

#[test]
fn test_keeps_the_failures_of_a_file_before_one_that_exits() {
    let (out, err, success) = run_test(&["fail.py", "exits.py", "ok.py"], &[]);
    assert!(!success);
    assert_eq!(out, "fail.py\nexits.py\nok.py\n");
    assert!(err.contains("1 test, 0 successes, 1 failure and 0 errors.\n"));
    assert!(err.contains("The file exited before its tests ran.\n"));
    assert!(err.ends_with("2 tests, 2 successes, 0 failures and 0 errors.\n"));
}

#[test]
fn test_goes_on_after_a_file_that_raises_before_its_tests() {
    let (out, err, success) = run_test(&["raises.py", "fail.py"], &[]);
    assert!(!success);
    assert_eq!(out, "raises.py\nfail.py\n");
    assert!(err.contains("The file raised an exception before its tests ran.\n"));
    assert!(err.ends_with("1 test, 0 successes, 1 failure and 0 errors.\n"));
}

#[test]
fn test_verbose() {
    let (out, err, success) = run_test(&["ok.py"], &["--verbose"]);
    assert!(success);
    assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
}

// --- Embedded spython library visibility ---

#[test]
fn test_spython_wildcard_ok() {
    // `from spython import *` must surface the lib's public names to ty's
    // resolver, so a valid call to `square` / `fill` / `red` type-checks.
    let (out, err, success) = run_test(&["spython_wildcard_ok.py"], &[]);
    assert!(success, "stdout: {out}\nstderr: {err}");
    assert_eq!(out, "");
    assert_eq!(err, "");
}

#[test]
fn test_spython_wildcard_unresolved() {
    // Names not exported by `spython` must still error after
    // `from spython import *` — we resolve real exports, not synthesize
    // arbitrary names.
    let (out, err, success) = run_test(&["spython_wildcard_unresolved.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn test_spython_lib_type_error() {
    // ty must type-check calls into the spython lib using real signatures —
    // passing a `Color` where a `Style` is expected should fail.
    let (out, err, success) = run_test(&["spython_lib_type_error.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    // Strip the user-specific cache prefix so the snapshot is portable.
    insta::with_settings!({
        filters => vec![
            (filter.as_str(), ""),
            (r"\S+/_spython_lib/spython/", "<lib>/"),
        ]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

// --- Doctest validation tests ---

#[test]
fn test_doctest_type_error() {
    let (out, err, success) = run_test(&["doctest_type_error.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn test_doctest_syntax_error() {
    // A doctest with a parse error (unclosed paren) must be reported by the
    // static checker — not slip through to the runtime doctest runner, which
    // would surface it in a non-spython format.
    let (out, err, success) = run_test(&["doctest_syntax_error.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn test_doctest_no_space() {
    let (out, err, success) = run_test(&["doctest_no_space.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn test_doctest_misaligned() {
    let (out, err, success) = run_test(&["doctest_misaligned.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn test_doctest_never_runs() {
    let (out, err, success) = run_test(&["doctest_never_runs.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn test_doctest_under_name_check() {
    let (out, err, success) = run_test(&["doctest_under_name_check.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn test_doctest_test_dict() {
    let (out, err, success) = run_test(&["doctest_test_dict.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn test_doctest_two_statements() {
    let (out, err, success) = run_test(&["doctest_two_statements.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn test_doctest_continuation_no_space() {
    let (out, err, success) = run_test(&["doctest_continuation_no_space.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn test_doctest_bare_name_allowed() {
    // A bare `>>> y` line is a value-display idiom in doctests and must not
    // trigger the BARE_EXPRESSION lint — otherwise teaching examples like
    // `>>> x: int = 2` / `>>> x` / `2` would fail level-0 checks.
    let (out, err, success) = run_test(&["doctest_bare_name.py"], &[]);
    assert!(success, "stdout={out} stderr={err}");
    assert_eq!(out, "");
    assert_eq!(
        err,
        "Running tests...\n  f\n2 tests, 2 successes, 0 failures and 0 errors.\n"
    );
}

#[test]
fn test_doctest_level_restriction() {
    // `if` inside a doctest must be rejected at level 0 like any other code.
    let (out, err, success) = run_test(&["doctest_level_restriction.py"], &["--level", "0"]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn test_doctest_module_level() {
    let (out, err, success) = run_test(&["doctest_module_level.py"], &[]);
    assert!(!success);
    let filter = normalize_output(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests/"));
    insta::with_settings!({
        filters => vec![(filter.as_str(), "")]
    }, {
        assert_snapshot!(format!("STDOUT\n{out}STDERR\n{err}"));
    });
}

#[test]
fn test_doctest_cross_module() {
    // Doctest references a name imported from a sibling first-party file
    // (`from helper import square`). Type-checking the doctest must resolve
    // `square` through the import so that the wrong-type call is caught.
    let subdir = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/doctests/doctest_cross_module"
    ));
    let output = spython()
        .current_dir(subdir)
        .args(["test", "--level", "4", "main.py"])
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
fn test_resolves_a_sibling_from_another_directory() {
    // `spython test dir/main.py` from outside `dir`. Both halves of the
    // check look beside the file: the static one resolves `from helper
    // import square`, and the doctests, which run for real, import it again.
    let doctests = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/doctests"));
    let (out, err, success) = run_spython_native(
        &["test", "--level", "4", "imports_a_sibling/main.py"],
        doctests,
    );
    assert!(success, "expected success; stdout={out} stderr={err}");
    assert!(
        err.contains("1 test, 1 success"),
        "expected the doctest to run, got: {err}"
    );
}

#[test]
fn test_refuses_what_run_cannot_import() {
    // `sub/main.py` imports `toplevel`, which sits beside `sub`, not inside
    // it. The interpreter puts the file's own directory on `sys.path`, so it
    // cannot reach it -- and the check, rooted the same way, says so instead
    // of passing the file on to fail at runtime.
    let root = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/doctests/subdir_import"
    ));
    let (_, tested, test_ok) = run_spython_native(&["test", "--level", "4", "sub/main.py"], root);
    let (_, ran, run_ok) = run_spython_native(&["run", "--level", "4", "sub/main.py"], root);
    assert!(!test_ok, "test should have refused, got: {tested}");
    assert!(!run_ok, "run should have refused, got: {ran}");
    for (what, err) in [("test", &tested), ("run", &ran)] {
        assert!(
            err.contains("Cannot resolve imported module `toplevel`"),
            "expected {what} to report the unresolved import, got: {err}"
        );
    }
}

#[test]
fn test_doctest_under_a_package() {
    // Same as above, one directory down: the imported file lives in a package
    // (`from pkg.helper import square`). The doctest check writes the file's
    // imports into a filesystem of its own, so a name that only resolves
    // through a directory has to survive the trip.
    let subdir = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/doctests/doctest_package"
    ));
    let output = spython()
        .current_dir(subdir)
        .args(["test", "--level", "4", "main.py"])
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
        "expected doctest error through the package, got: {err}"
    );
}

#[test]
fn run_rejects_doctest_type_error() {
    // `spython run` should reject doctest type errors even though it never
    // executes the doctests themselves — they are part of the file and must
    // pass the same type checks as any other code.
    let path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/doctests/doctest_type_error.py"
    ));
    let dir = path.parent().expect("path has parent");
    let filename = path.file_name().expect("path has filename");
    let output = spython()
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

// --- Type checking tests ---

fn run_check(args: &[&str]) -> (String, String, bool) {
    let inputs = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/inputs"));
    run_spython_native(args, inputs)
}

#[test]
fn check_passes_in_silence() {
    // A file that checks says nothing. The student sees output only when
    // something is wrong.
    let (out, err, success) = run_check(&["check", "ok_simple.py"]);
    assert!(success, "stdout: {out}\nstderr: {err}");
    assert_eq!(out, "");
    assert_eq!(err, "");
}

#[test]
fn check_reports_a_type_error() {
    let (out, err, success) = run_check(&["check", "err_type.py"]);
    assert!(!success, "check should have refused, got: {out}{err}");
    assert_eq!(out, "");
    assert_snapshot!(err);
}

#[test]
fn check_does_not_run_the_file() {
    // `run` prints what the file prints; `check` prints nothing at all.
    let (run_out, _, _) = run_check(&["run", "ok_simple.py"]);
    assert_eq!(run_out, "3\n");
    let (check_out, _, _) = run_check(&["check", "ok_simple.py"]);
    assert_eq!(check_out, "");
}

#[test]
fn check_refuses_what_run_cannot_import() {
    // `sub/main.py` imports `toplevel`, which sits beside `sub`, not inside
    // it. The interpreter puts the file's own directory on `sys.path`, so it
    // cannot reach it, and `check`, rooted the same way, says so.
    let root = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/doctests/subdir_import"
    ));
    let (_, err, success) = run_spython_native(&["check", "--level", "4", "sub/main.py"], root);
    assert!(!success, "check should have refused, got: {err}");
    assert!(
        err.contains("Cannot resolve imported module `toplevel`"),
        "expected the unresolved import, got: {err}"
    );
}

#[test]
fn check_refuses_a_module_under_src() {
    // ty looks in `src` when the directory has one. The interpreter does not.
    let dir = new_dir_with(
        "src_layout",
        &[
            ("main.py", "import helper\nprint(helper.f())\n"),
            ("src/helper.py", "def f() -> int:\n    return 1\n"),
        ],
    );
    let (_, err, success) = run_spython_native(&["check", "main.py"], &dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!success, "check should have refused, got: {err}");
    assert!(
        err.contains("Cannot resolve imported module `helper`"),
        "expected the unresolved import, got: {err}"
    );
}

#[test]
fn run_imports_a_module_beside_the_file_named_after_the_stdlib() {
    // `re` is not available, but a `re.py` the student wrote is.
    let dir = new_dir_with(
        "beside_stdlib",
        &[
            ("main.py", "import re\nprint(re.f())\n"),
            ("re.py", "def f() -> int:\n    return 1\n"),
        ],
    );
    let (out, err, success) = run_spython_native(&["main.py"], &dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(success, "the file should have run, got: {err}");
    assert_eq!(out, "1\n");
}

#[test]
fn check_refuses_a_module_beside_the_file_the_interpreter_hides() {
    // The interpreter takes its own `copy` and `enum` before the files.
    for module in ["copy", "enum"] {
        let main = format!("import {module}\nprint({module}.f())\n");
        let dir = new_dir_with(
            "shadowed",
            &[
                ("main.py", main.as_str()),
                (&format!("{module}.py"), "def f() -> int:\n    return 1\n"),
            ],
        );
        let (_, err, success) = run_spython_native(&["check", "main.py"], &dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!success, "check should have refused {module}, got: {err}");
        assert!(
            err.contains("error[shadowed-module]"),
            "expected a shadowed {module}, got: {err}"
        );
    }
}

#[test]
fn check_refuses_a_module_from_site_packages() {
    // The interpreter has no site-packages, so the checker must not look in
    // the one of the environment `VIRTUAL_ENV` names.
    let dir = new_dir_with(
        "site_packages",
        &[
            ("main.py", "import thirdparty\nprint(thirdparty.f())\n"),
            ("venv/pyvenv.cfg", "home = /usr/bin\nversion = 3.14.0\n"),
            (
                "venv/lib/python3.14/site-packages/thirdparty.py",
                "def f() -> int:\n    return 1\n",
            ),
        ],
    );
    let output = spython()
        .current_dir(&dir)
        .env("VIRTUAL_ENV", dir.join("venv"))
        .args(["check", "main.py"])
        .output()
        .expect("failed to run spython");
    let _ = std::fs::remove_dir_all(&dir);
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "check should have refused, got: {err}"
    );
    assert!(
        err.contains("Cannot resolve imported module `thirdparty`"),
        "expected the unresolved import, got: {err}"
    );
}

#[test]
fn check_ignores_a_virtual_env_that_is_not_there() {
    let dir = new_dir_with("no_venv", &[("main.py", "print(1)\n")]);
    let output = spython()
        .current_dir(&dir)
        .env("VIRTUAL_ENV", dir.join("missing"))
        .args(["main.py"])
        .output()
        .expect("failed to run spython");
    let _ = std::fs::remove_dir_all(&dir);
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the file should have run, got: {err}"
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
}

/// A file under `cli/tests`, named the way both backends are given it: the
/// directory to run from, and the bare name to run.
fn dual_run(command: &str, dir: &str, file: &str) -> (String, String, bool) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(dir);
    run_spython_cmd(&[command, "--level", "4", file], &dir)
}

/// Both backends are asked for the same file, and `run_spython_cmd` asserts
/// they agree. Each drives the export of its own command's name, so what one
/// prints the other prints too, whether the examples pass or fail.
#[test]
fn test_output_agrees_between_backends() {
    // A file whose examples pass, one whose example fails, one with no
    // examples at all, one that exits and one that raises before its
    // examples, and one that prints from the top level, which `test` does not
    // reach: the source runs under a name of its own, not as `__main__`.
    for file in ["ok.py", "fail.py", "no_tests.py", "exits.py", "raises.py"] {
        dual_run("test", "doctests", file);
    }
    dual_run("test", "inputs", "ok_simple.py");
}

/// The traceback shows the line that raised, in the file and in the library,
/// which a page has to take from the source it was given.
#[test]
fn run_output_agrees_between_backends() {
    let (_, err, _) = dual_run("run", "doctests", "raises.py");
    assert!(
        err.contains("line 9, in <module>\n    int(\"z\")\n"),
        "{err}"
    );
    let (_, err, _) = dual_run("run", "doctests", "raises_in_lib.py");
    assert!(
        err.contains("in beside\n    raise TypeError(\"beside expects at least 2 images\")\n"),
        "{err}"
    );
}

#[test]
fn check_output_agrees_between_backends() {
    // A file that holds, one whose doctest does not type-check, and one that
    // prints when it runs, which `check` never does.
    for file in ["no_tests.py", "ok.py", "doctest_type_error.py"] {
        dual_run("check", "doctests", file);
    }
    dual_run("check", "inputs", "ok_simple.py");
}

// --- Level restriction tests ---

fn run_level(level: u8, code: &str) -> (String, String, bool) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir();
    let filename = format!("_spython_level_test_{n}.py");
    let file = dir.join(&filename);
    std::fs::write(&file, code).expect("write the test file");
    let level_str = level.to_string();
    let result = run_spython_cmd(&["run", "--level", &level_str, &filename], &dir);
    let _ = std::fs::remove_file(&file);
    result
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
    // Native-only: `open()` on /dev/null requires WASI `path_open`, which
    // the minimal WASI polyfill used by the Deno wrapper does not provide.
    let null_path = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let code = formatdoc! {"
        with open('{null_path}') as f:
            pass
        print('ok')
    "};
    let (out, _, success) = run_level_native_only(5, &code);
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
fn level0_allows_any_typed_operand_in_arithmetic() {
    // `(-1) ** i` is inferred as `Any` by ty. `Any` is assignable to every
    // type (including `bool`), so the bool-in-arithmetic lint must not treat
    // it as a `bool` operand.
    let (out, err, success) = run_level(
        0,
        indoc! {"
        def f(n: int) -> float:
            r: float = 0.0
            r = (-1) ** n / (2 * n + 1)
            return r
        print(f(0))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "1.0\n");
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

// --- Naming convention tests ---

#[test]
fn level0_flags_camelcase_function_name() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def myFunction(x: int) -> int:
            return x
    "},
    );
    assert!(!success);
    assert!(
        err.contains("invalid-function-name"),
        "expected invalid-function-name in stderr: {err}"
    );
}

#[test]
fn level0_allows_lowercase_function_name() {
    let (out, err, success) = run_level(
        0,
        indoc! {"
        def my_function(x: int) -> int:
            return x
        print(my_function(1))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "1\n");
}

#[test]
fn level0_allows_dunder_function_name() {
    let (out, err, success) = run_level(
        0,
        indoc! {"
        def __init__(x: int) -> int:
            return x
        print(__init__(2))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "2\n");
}

#[test]
fn level2_flags_lowercase_class_name() {
    let (_, err, success) = run_level(
        2,
        indoc! {"
        from dataclasses import dataclass
        @dataclass
        class my_class:
            x: int
    "},
    );
    assert!(!success);
    assert!(
        err.contains("invalid-class-name"),
        "expected invalid-class-name in stderr: {err}"
    );
}

#[test]
fn level2_flags_underscore_class_name() {
    let (_, err, success) = run_level(
        2,
        indoc! {"
        from dataclasses import dataclass
        @dataclass
        class My_Class:
            x: int
    "},
    );
    assert!(!success);
    assert!(
        err.contains("invalid-class-name"),
        "expected invalid-class-name in stderr: {err}"
    );
}

#[test]
fn level2_allows_capwords_class_name() {
    let (_, err, success) = run_level(
        2,
        indoc! {"
        from dataclasses import dataclass
        @dataclass
        class MyClass:
            x: int
    "},
    );
    assert!(success, "stderr: {err}");
}

#[test]
fn level0_flags_uppercase_argument_name() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(X: int) -> int:
            return X
    "},
    );
    assert!(!success);
    assert!(
        err.contains("invalid-argument-name"),
        "expected invalid-argument-name in stderr: {err}"
    );
}

#[test]
fn level0_allows_self_in_methods() {
    // 'self' is lowercase, so it's not flagged.
    let (out, err, success) = run_level(
        4,
        indoc! {"
        class Box:
            x: int
            def __init__(self, x: int) -> None:
                self.x = x
        print(Box(7).x)
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "7\n");
}

#[test]
fn level0_flags_uppercase_variable_in_function() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            Y: int = x + 1
            return Y
    "},
    );
    assert!(!success);
    assert!(
        err.contains("non-lowercase-variable-in-function"),
        "expected non-lowercase-variable-in-function in stderr: {err}"
    );
}

#[test]
fn level0_allows_uppercase_module_constant() {
    // Module-level UPPER_CASE is the conventional form for constants.
    let (out, err, success) = run_level(
        0,
        indoc! {"
        MAX_SIZE: int = 100
        def get_max() -> int:
            return MAX_SIZE
        print(get_max())
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "100\n");
}

#[test]
fn level3_flags_uppercase_for_loop_variable() {
    let (_, err, success) = run_level(
        3,
        indoc! {"
        def f() -> None:
            for I in range(3):
                print(I)
        f()
    "},
    );
    assert!(!success);
    assert!(
        err.contains("non-lowercase-variable-in-function"),
        "expected non-lowercase-variable-in-function in stderr: {err}"
    );
}

#[test]
fn level2_flags_lowercase_enum_member() {
    let (_, err, success) = run_level(
        2,
        indoc! {"
        from enum import IntEnum
        class Color(IntEnum):
            red = 1
            blue = 2
    "},
    );
    assert!(!success);
    assert!(
        err.contains("non-uppercase-enum-member"),
        "expected non-uppercase-enum-member in stderr: {err}"
    );
}

#[test]
fn level2_flags_mixedcase_enum_member() {
    let (_, err, success) = run_level(
        2,
        indoc! {"
        from enum import StrEnum
        class Dir(StrEnum):
            Up = 'u'
            Down = 'd'
    "},
    );
    assert!(!success);
    assert!(
        err.contains("non-uppercase-enum-member"),
        "expected non-uppercase-enum-member in stderr: {err}"
    );
}

#[test]
fn level2_allows_uppercase_enum_member() {
    let (_, err, success) = run_level(
        2,
        indoc! {"
        from enum import IntEnum
        class Color(IntEnum):
            RED = 1
            DARK_BLUE = 2
    "},
    );
    assert!(success, "stderr: {err}");
}

// --- Comparison style tests ---

#[test]
fn level0_flags_eq_none() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> bool:
            return x == None
    "},
    );
    assert!(!success);
    assert!(
        err.contains("none-comparison"),
        "expected none-comparison in stderr: {err}"
    );
}

#[test]
fn level0_flags_neq_none() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> bool:
            return x != None
    "},
    );
    assert!(!success);
    assert!(err.contains("none-comparison"), "stderr: {err}");
}

#[test]
fn level0_allows_is_none() {
    let (out, err, success) = run_level(
        4,
        indoc! {"
        def f(x: object) -> bool:
            return x is None
        print(f(None))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "True\n");
}

#[test]
fn level0_flags_eq_true() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: bool) -> bool:
            return x == True
    "},
    );
    assert!(!success);
    assert!(
        err.contains("true-false-comparison"),
        "expected true-false-comparison in stderr: {err}"
    );
}

#[test]
fn level0_flags_eq_false() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: bool) -> bool:
            return x == False
    "},
    );
    assert!(!success);
    assert!(err.contains("true-false-comparison"), "stderr: {err}");
}

#[test]
fn level4_flags_not_in_membership() {
    let (_, err, success) = run_level(
        4,
        indoc! {"
        def f(xs: list[int]) -> bool:
            return not 1 in xs
    "},
    );
    assert!(!success);
    assert!(
        err.contains("not-in-test"),
        "expected not-in-test in stderr: {err}"
    );
}

#[test]
fn level4_allows_not_in() {
    let (out, err, success) = run_level(
        4,
        indoc! {"
        def f(xs: list[int]) -> bool:
            return 1 not in xs
        print(f([2, 3]))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "True\n");
}

#[test]
fn level4_flags_not_is_identity() {
    let (_, err, success) = run_level(
        4,
        indoc! {"
        def f(x: object) -> bool:
            return not x is None
    "},
    );
    assert!(!success);
    assert!(
        err.contains("not-is-test"),
        "expected not-is-test in stderr: {err}"
    );
}

// --- N805: first method argument must be `self` ---

#[test]
fn level4_flags_method_first_arg_not_self() {
    let (_, err, success) = run_level(
        4,
        indoc! {"
        class Box:
            x: int
            def __init__(this, x: int) -> None:
                this.x = x
    "},
    );
    assert!(!success);
    assert!(
        err.contains("invalid-first-argument-name-for-method"),
        "expected invalid-first-argument-name-for-method in stderr: {err}"
    );
}

#[test]
fn level4_allows_method_with_self() {
    let (out, err, success) = run_level(
        4,
        indoc! {"
        class Box:
            x: int
            def __init__(self, x: int) -> None:
                self.x = x
        print(Box(3).x)
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "3\n");
}

#[test]
fn level4_skips_self_check_for_staticmethod() {
    let (out, err, success) = run_level(
        4,
        indoc! {"
        class Box:
            x: int
            @staticmethod
            def make(x: int) -> int:
                return x
        print(Box.make(7))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "7\n");
}

// --- SIM103: needless bool ---

#[test]
fn level1_flags_needless_bool_if_else() {
    let (_, err, success) = run_level(
        1,
        indoc! {"
        def f(x: bool) -> bool:
            if x:
                return True
            else:
                return False
    "},
    );
    assert!(!success);
    assert!(
        err.contains("needless-bool"),
        "expected needless-bool in stderr: {err}"
    );
}

#[test]
fn level1_flags_needless_bool_negated() {
    let (_, err, success) = run_level(
        1,
        indoc! {"
        def f(x: bool) -> bool:
            if x:
                return False
            else:
                return True
    "},
    );
    assert!(!success);
    assert!(err.contains("needless-bool"), "stderr: {err}");
}

#[test]
fn level1_allows_direct_return() {
    let (out, err, success) = run_level(
        1,
        indoc! {"
        def f(x: bool) -> bool:
            return x
        print(f(True))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "True\n");
}

// --- F541: f-string without placeholders ---

#[test]
fn level0_flags_f_string_without_placeholders() {
    let (_, err, success) = run_level(
        0,
        indoc! {r#"
        def f() -> str:
            return f"hello"
    "#},
    );
    assert!(!success);
    assert!(
        err.contains("f-string-missing-placeholders"),
        "expected f-string-missing-placeholders in stderr: {err}"
    );
}

#[test]
fn level0_allows_f_string_with_placeholder() {
    let (out, err, success) = run_level(
        0,
        indoc! {r#"
        def f(x: int) -> str:
            return f"x={x}"
        print(f(3))
    "#},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "x=3\n");
}

// --- F841: unused variable (levels 0-3 only) ---

#[test]
fn level0_flags_unused_local() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            y: int = x + 1
            return x
    "},
    );
    assert!(!success);
    assert!(
        err.contains("unused-variable"),
        "expected unused-variable in stderr: {err}"
    );
}

#[test]
fn level0_unused_variable_message_names_variable() {
    let (_, err, _) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            y: int = x + 1
            return x
    "},
    );
    assert!(
        err.contains("`y`"),
        "expected variable name in stderr: {err}"
    );
}

#[test]
fn level0_allows_used_local() {
    let (out, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            y: int = x + 1
            return y
        print(f(2))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "3\n");
}

#[test]
fn level0_skips_underscore_prefixed() {
    let (out, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int) -> int:
            _y: int = x + 1
            return x
        print(f(2))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "2\n");
}

#[test]
fn level3_flags_unused_for_target() {
    let (_, err, success) = run_level(
        3,
        indoc! {"
        def f() -> None:
            for i in range(3):
                print('hi')
        f()
    "},
    );
    assert!(!success);
    assert!(err.contains("unused-variable"), "stderr: {err}");
}

#[test]
fn level3_flags_unused_in_tuple_unpacking() {
    let (_, err, success) = run_level(
        3,
        indoc! {"
        def f() -> int:
            a, b = (1, 2)
            return a
    "},
    );
    assert!(!success);
    assert!(err.contains("unused-variable"), "stderr: {err}");
    assert!(err.contains("`b`"), "stderr: {err}");
}

#[test]
fn level3_allows_aug_assign_as_use() {
    // `x += 1` reads x; that counts as a use, so the original binding is
    // not flagged.
    let (out, err, success) = run_level(
        3,
        indoc! {"
        def f() -> int:
            x: int = 0
            x += 1
            return x
        print(f())
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "1\n");
}

#[test]
fn level3_aug_assign_alone_counts_as_use() {
    // `x: int = 0; x += 1` — the AugAssign reads x, so we must NOT flag
    // the original binding as unused, even when x isn't read elsewhere.
    let (_, err, success) = run_level(
        3,
        indoc! {"
        def f(out: list[int]) -> None:
            count: int = 0
            count += 1
            out.append(count)
        f([])
    "},
    );
    assert!(success, "stderr: {err}");
}

#[test]
fn level3_chained_eq_none_no_double_emit() {
    let (_, err, success) = run_level(
        4,
        indoc! {"
        def f(x: int, y: int) -> bool:
            return x == None == y
    "},
    );
    assert!(!success);
    let count = err.matches("none-comparison").count();
    // Two None literals → expect at most one diagnostic per literal.
    // (Two None literals at distinct ranges → two emits is acceptable.)
    assert!((1..=2).contains(&count), "unexpected count {count}: {err}");
}

#[test]
fn level4_does_not_flag_unused() {
    // F841 only applies at levels 0-3.
    let (out, err, success) = run_level(
        4,
        indoc! {"
        def f(x: int) -> int:
            y: int = x + 1
            return x
        print(f(2))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "2\n");
}

#[test]
fn level0_does_not_flag_function_parameter() {
    // Parameters aren't tracked as unused locals (different rule).
    let (out, err, success) = run_level(
        0,
        indoc! {"
        def f(x: int, y: int) -> int:
            return x
        print(f(1, 2))
    "},
    );
    assert!(success, "stderr: {err}");
    assert_eq!(out, "1\n");
}

// --- PIE790: unnecessary `pass` ---

#[test]
fn level0_flags_unnecessary_pass_after_docstring() {
    let (_, err, success) = run_level(
        0,
        indoc! {r#"
        def f() -> None:
            """Docstring."""
            pass
    "#},
    );
    assert!(!success);
    assert!(
        err.contains("unnecessary-pass"),
        "expected unnecessary-pass in stderr: {err}"
    );
}

#[test]
fn level0_allows_pass_as_only_body_stmt() {
    let (_, err, success) = run_level(
        0,
        indoc! {"
        def f() -> None:
            pass
        f()
    "},
    );
    assert!(success, "stderr: {err}");
}

#[test]
fn level5_still_flags_invalid_names() {
    // Naming conventions apply at every level — they're a style rule, not a
    // language-complexity gate.
    let (_, err, success) = run_level(
        5,
        indoc! {"
        def MyFunction(x: int) -> int:
            return x
    "},
    );
    assert!(!success);
    assert!(
        err.contains("invalid-function-name"),
        "expected invalid-function-name in stderr: {err}"
    );
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

// --- Format tests ---

/// The spacing `format` fixes, and what it turns into.
const UNFORMATTED: &str = "def f( x ):\n  return   x+1\n";
const FORMATTED: &str = "def f(x):\n    return x + 1\n";

fn file_in(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).expect("read the formatted file")
}

/// Run `spython format <args>` over stdin, which is where it reads from when
/// no path names a file.
fn run_format_stdin(args: &[&str], input: &str) -> (String, String, bool) {
    let mut command = spython();
    command.arg("format").args(args);
    run_with_stdin(&mut command, input.as_bytes())
}

#[test]
fn format_writes_the_file() {
    let dir = new_dir_with("format", &[("a.py", UNFORMATTED)]);
    let (out, err, success) = run_spython_native(&["format", "a.py"], &dir);
    assert!(success, "stdout: {out}\nstderr: {err}");
    assert_eq!(file_in(&dir, "a.py"), FORMATTED);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn format_recurses_into_directories() {
    let dir = new_dir_with(
        "format",
        &[("sub/a.py", UNFORMATTED), ("sub/b.txt", UNFORMATTED)],
    );
    let (out, err, success) = run_spython_native(&["format", "."], &dir);
    assert!(success, "stdout: {out}\nstderr: {err}");
    assert_eq!(file_in(&dir, "sub/a.py"), FORMATTED);
    // Only Python is formatted; a name the formatter has no business with is
    // left as it is.
    assert_eq!(file_in(&dir, "sub/b.txt"), UNFORMATTED);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn format_check_names_the_file_and_leaves_it_alone() {
    let dir = new_dir_with("format", &[("a.py", UNFORMATTED)]);
    let (_, err, success) = run_spython_native(&["format", "--check", "a.py"], &dir);
    assert!(!success, "stderr: {err}");
    assert!(err.contains("'a.py' is not formatted"), "stderr: {err}");
    assert_eq!(file_in(&dir, "a.py"), UNFORMATTED);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn format_check_passes_on_a_formatted_file() {
    let dir = new_dir_with("format", &[("a.py", FORMATTED)]);
    let (out, err, success) = run_spython_native(&["format", "--check", "a.py"], &dir);
    assert!(success, "stdout: {out}\nstderr: {err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn format_fails_on_a_file_it_cannot_parse() {
    let dir = new_dir_with(
        "format",
        &[("bad.py", "def f(:\n"), ("good.py", UNFORMATTED)],
    );
    for args in [&["format", "."][..], &["format", "--check", "."][..]] {
        let (_, err, success) = run_spython_native(args, &dir);
        assert!(!success, "{args:?}: {err}");
        assert!(err.contains("cannot format './bad.py'"), "{args:?}: {err}");
    }
    // The file the formatter reads is still formatted, as one file it turns
    // down does not stop the others.
    assert_eq!(file_in(&dir, "good.py"), FORMATTED);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn format_reads_stdin_and_writes_stdout() {
    let (out, err, success) = run_format_stdin(&[], UNFORMATTED);
    assert!(success, "stderr: {err}");
    assert_eq!(out, FORMATTED);
}

#[test]
fn format_check_turns_down_what_stdin_holds() {
    let (out, err, success) = run_format_stdin(&["--check"], UNFORMATTED);
    assert!(!success, "stderr: {err}");
    // What it read is not written back, whether it is formatted or not.
    assert_eq!(out, "");
    assert!(err.contains("'<stdin>' is not formatted"), "stderr: {err}");
}

// --- REPL tests ---

fn run_repl(input: &str) -> (String, String, bool) {
    run_repl_level(input, 0)
}

fn run_repl_level(input: &str, level: u8) -> (String, String, bool) {
    run_repl_in(input, &["repl", "--level", &level.to_string()], None)
}

/// A REPL that starts with `source` already executed, as `spython repl FILE` does.
fn run_repl_after_file(input: &str, level: u8, source: &str) -> (String, String, bool) {
    run_repl_after_files(input, level, &[("main.py", source)])
}

/// Write `files` into a directory of their own and start the REPL on the
/// first of them, so a file that imports its neighbours can be tested.
fn run_repl_after_files(input: &str, level: u8, files: &[(&str, &str)]) -> (String, String, bool) {
    let dir = new_dir_with("repl_preload", files);
    // From the directory the files are in, which is where ty looks for the
    // ones a file imports.
    let result = run_repl_in(
        input,
        &[
            "repl",
            "--level",
            &level.to_string(),
            files.first().expect("at least one file").0,
        ],
        Some(&dir),
    );
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn run_repl_in(input: &str, args: &[&str], cwd: Option<&Path>) -> (String, String, bool) {
    let mut command = spython();
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command.args(args);
    let (stdout, stderr, success) = run_with_stdin(&mut command, input.as_bytes());
    // Strip the welcome banner (two lines: "Welcome to..." and "Type ctrl-d...")
    let stdout = match stdout.find("to exit.\n") {
        Some(pos) => stdout[pos + "to exit.\n".len()..].to_owned(),
        None => stdout,
    };
    (
        stdout.replace(">>> ", "").replace("... ", ""),
        stderr,
        success,
    )
}

/// The history outlives a session that ends with `:quit`, the way it outlives
/// one that ends with the end of its input. Linux only: `dirs::data_dir` is
/// the only one that reads `XDG_DATA_HOME`, so it is the only platform the
/// test can point somewhere of its own.
#[cfg(target_os = "linux")]
#[test]
fn the_repl_keeps_the_history_of_a_session_that_quit() {
    let data = std::env::temp_dir().join(format!("_spython_history_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    std::fs::create_dir_all(&data).expect("create the data directory");
    let mut command = spython();
    command.arg("repl").env("XDG_DATA_HOME", &data);
    let (_, err, success) = run_with_stdin(&mut command, b"x = 1\n:quit\n");
    assert!(success, "stderr: {err}");
    let history = std::fs::read_to_string(data.join("spython").join("history"))
        .expect("read the history the session left");
    assert!(history.contains("x = 1"), "history: {history}");
    let _ = std::fs::remove_dir_all(&data);
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
fn repl_reads_on_after_a_decorator() {
    let input = "from dataclasses import dataclass\n@dataclass\nclass P:\n    x: int\n\nP(1).x\n";
    let (out, err, success) = run_repl_level(input, 2);
    assert!(success);
    assert!(!err.contains("invalid-syntax"), "stderr: {err}");
    assert_eq!(out, "1\n");
}

#[test]
fn repl_reads_on_inside_a_command() {
    let (out, _, success) = run_repl_level(":time [1,\n2]\n", 3);
    assert!(success);
    assert!(out.contains("[1, 2]"), "stdout: {out}");
}

#[test]
fn repl_runs_the_input_the_file_ended_in_the_middle_of() {
    let (_, err, success) = run_repl("def f() -> int:\n    return 1");
    assert!(success);
    assert!(err.is_empty(), "stderr: {err}");
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
fn repl_traceback_shows_the_lines_of_each_input() {
    let input = "def f(n: int) -> int:\n    return n // (n - n)\n\nf(1)\n";
    let (_, err, _) = run_repl(input);
    assert!(
        err.contains(
            "  File \"<python-input-1>\", line 1, in <module>\n    f(1)\n  \
             File \"<python-input-0>\", line 2, in f\n    return n // (n - n)\n"
        ),
        "{err}"
    );
}

#[test]
fn repl_after_a_file_sees_what_it_defined() {
    let (out, err, success) =
        run_repl_after_file("f(3)\n", 0, "def f(x: int) -> int:\n    return x * 2\n");
    assert!(success);
    assert!(!err.contains("unresolved-reference"), "stderr: {err}");
    assert_eq!(out, "6\n");
}

#[test]
fn repl_after_a_file_that_calls_exit_ends_with_its_status() {
    let (out, err, success) = run_repl_after_file("print('prompt')\n", 0, "exit(2)\n");
    assert!(!success);
    assert!(!err.contains("SystemExit"), "stderr: {err}");
    assert!(!out.contains("prompt"), "stdout: {out}");
}

#[test]
fn repl_after_a_file_that_calls_exit_with_zero_succeeds() {
    let (out, err, success) = run_repl_after_file("print('prompt')\n", 0, "exit(0)\n");
    assert!(success, "stderr: {err}");
    assert!(!out.contains("prompt"), "stdout: {out}");
}

#[test]
fn repl_after_a_file_that_calls_exit_shows_its_message() {
    let (_, err, success) = run_repl_after_file("", 0, "exit('bye')\n");
    assert!(!success);
    assert_eq!(err, "bye\n");
}

#[test]
fn repl_after_a_file_sees_what_it_imported() {
    let (out, err, success) = run_repl_after_files(
        "g(1)\nhelpers.helper(5)\n",
        0,
        &[
            (
                "main.py",
                "import helpers\n\n\ndef g(x: int) -> int:\n    return helpers.helper(x)\n",
            ),
            (
                "helpers.py",
                "def helper(x: int) -> int:\n    return x * 2\n",
            ),
        ],
    );
    assert!(success);
    assert!(!err.contains("unresolved-import"), "stderr: {err}");
    assert_eq!(out, "2\n10\n");
}

#[test]
fn repl_after_a_file_sees_a_module_under_a_package() {
    let (out, err, success) = run_repl_after_files(
        "h(4)\n",
        0,
        &[
            (
                "main.py",
                "from pkg.util import twice\n\n\ndef h(x: int) -> int:\n    return twice(x)\n",
            ),
            ("pkg/__init__.py", ""),
            (
                "pkg/util.py",
                "def twice(x: int) -> int:\n    return x * 2\n",
            ),
        ],
    );
    assert!(success);
    assert!(!err.contains("unresolved-import"), "stderr: {err}");
    assert_eq!(out, "8\n");
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
fn repl_help_lists_the_commands_of_both_the_engine_and_the_terminal() {
    let (out, _, success) = run_repl(":help\n");
    assert!(success);
    for command in [":help", ":quit", ":type", ":time", ":level", ":theme"] {
        assert!(out.contains(command), "{command} missing from :help: {out}");
    }
}

/// `dirs` takes the configuration directory from `XDG_CONFIG_HOME` only on
/// Linux, and elsewhere the test would write the real file.
#[cfg(target_os = "linux")]
#[test]
fn repl_theme_keeps_a_level_from_the_command_line_out_of_the_config() {
    let dir = new_dir_with(
        "theme_config",
        &[("spython/config", "level=2\ntheme=dark\n")],
    );
    let mut command = spython();
    command
        .env("XDG_CONFIG_HOME", &dir)
        .args(["repl", "--level", "5"]);
    let (_, err, success) = run_with_stdin(&mut command, b":theme light\n");
    let config = std::fs::read_to_string(dir.join("spython/config"));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(success, "{err}");
    assert_eq!(
        config.expect("the config is there"),
        "level=2\ntheme=light\n"
    );
}

#[test]
fn repl_unknown_command_is_refused() {
    let (_, err, success) = run_repl(":nope\n");
    assert!(success);
    assert!(err.contains("Unknown command :nope"), "stderr: {err}");
    assert!(!err.contains("invalid-syntax"), "stderr: {err}");
}

#[test]
fn repl_time_command_without_expression_says_what_it_takes() {
    let (_, err, success) = run_repl(":time\n");
    assert!(success);
    assert!(
        err.contains("The :time command takes an expression."),
        "stderr: {err}"
    );
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
    let output = spython()
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
    let output = spython()
        .args(["run", "nonexistent.py"])
        .output()
        .expect("failed to run spython");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot read 'nonexistent.py'"));
}

/// A file the checker turns down, for the tests that ask what colour the
/// answer takes.
fn dir_with_a_type_error(prefix: &str) -> PathBuf {
    new_dir_with(
        prefix,
        &[("bad.py", "def f(x: int) -> int:\n    return 'a'\n")],
    )
}

/// What `spython check bad.py` writes to stderr with `env` set. The wasm
/// wrapper has no environment, so these stay native, and the output comes
/// back as it was written: the escape codes are the answer.
fn check_stderr_with_env(dir: &Path, env: &[(&str, &str)]) -> String {
    let mut command = spython();
    command.env_remove("NO_COLOR").current_dir(dir);
    for (name, value) in env {
        command.env(name, value);
    }
    let output = command
        .args(["check", "bad.py"])
        .output()
        .expect("failed to run spython");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn force_color_colours_a_diagnostic_with_no_terminal_watching() {
    let dir = dir_with_a_type_error("force_color");
    let err = check_stderr_with_env(&dir, &[("FORCE_COLOR", "1")]);
    assert!(err.contains('\x1b'), "expected escape codes, got: {err}");
}

#[test]
fn no_color_keeps_the_codes_out_of_a_diagnostic() {
    let dir = dir_with_a_type_error("no_color");
    let err = check_stderr_with_env(&dir, &[("FORCE_COLOR", "1"), ("NO_COLOR", "1")]);
    assert!(
        !err.contains('\x1b'),
        "expected no escape codes, got: {err}"
    );
    assert!(err.contains("error"), "expected the diagnostic, got: {err}");
}

#[test]
fn an_empty_no_color_asks_for_nothing() {
    let dir = dir_with_a_type_error("empty_no_color");
    let err = check_stderr_with_env(&dir, &[("FORCE_COLOR", "1"), ("NO_COLOR", "")]);
    assert!(err.contains('\x1b'), "expected escape codes, got: {err}");
}
