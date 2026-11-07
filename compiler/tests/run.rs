//! Each test compiles a program, runs it in wasmtime and in CPython, and
//! compares what the two print and how they exit.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use engine::Level;
use engine::check::check_source;

/// The module of `source`, which has to pass the checker at the last level.
/// A traceback names the file by `path`.
fn compile(source: &str, path: &Path) -> Result<Vec<u8>, compiler::Error> {
    let checked = match check_source(source, Level::Full, false) {
        Ok(checked) => checked,
        Err(errors) => panic!("the checker rejects the program: {:?}", errors.diagnostics),
    };
    let path = path.to_str().expect("a path in utf-8");
    compiler::compile(&checked.db, checked.file, path)
}

/// The path of a file for the test `name`. Tests run at the same time, so
/// each one has its own.
fn path(name: &str, extension: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.{extension}"))
}

/// Runs `module` in the wasmtime on the `PATH`, or in the one that
/// `WASMTIME` names.
fn wasmtime(name: &str, module: &[u8]) -> Output {
    let path = path(name, "wasm");
    std::fs::write(&path, module).expect("the module written to disk");
    let wasmtime = std::env::var("WASMTIME").unwrap_or_else(|_| "wasmtime".to_owned());
    let mut command = Command::new(wasmtime);
    command
        .args(["run", "-W", "gc=y,function-references=y,exceptions=y"])
        .args(["--invoke", "_start"])
        .arg(&path);
    output_within_limit(command)
}

/// Runs `source` in the python3 on the `PATH`, or in the one that `PYTHON`
/// names.
fn cpython(name: &str, source: &str) -> Output {
    let path = path(name, "py");
    std::fs::write(&path, source).expect("the program written to disk");
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_owned());
    let mut command = Command::new(python);
    command.arg(&path);
    output_within_limit(command)
}

/// Runs `command` and collects what it prints. A test fails when the
/// program runs for more than 10 seconds, which is a program that never
/// ends, so the other tests still run.
fn output_within_limit(mut command: Command) -> Output {
    let limit = Duration::from_secs(10);
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the program to start");
    // Each pipe has its own reader, so a program that fills one pipe does
    // not stop while the test waits on the other.
    let stdout = read_in_thread(child.stdout.take().expect("a piped stdout"));
    let stderr = read_in_thread(child.stderr.take().expect("a piped stderr"));
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("the status of the program") {
            break status;
        }
        if start.elapsed() > limit {
            child.kill().expect("the program to stop");
            child.wait().expect("the program to stop");
            panic!("{command:?} ran for more than {limit:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Output {
        status,
        stdout: stdout.join().expect("the reader of stdout"),
        stderr: stderr.join().expect("the reader of stderr"),
    }
}

fn read_in_thread(mut pipe: impl Read + Send + 'static) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes)
            .expect("the output of the program");
        bytes
    })
}

/// Asserts that the compiled `source` prints what CPython prints, and exits
/// as CPython exits.
fn same_as_cpython(name: &str, source: &str) {
    let ours = run(name, source);
    let theirs = cpython(name, source);
    assert_eq!(
        String::from_utf8_lossy(&ours.stdout),
        String::from_utf8_lossy(&theirs.stdout),
        "stdout"
    );
    assert_eq!(
        String::from_utf8_lossy(&ours.stderr),
        String::from_utf8_lossy(&theirs.stderr),
        "stderr"
    );
    assert_eq!(ours.status.code(), theirs.status.code(), "exit code");
}

/// Compiles `source` and runs it in wasmtime.
fn run(name: &str, source: &str) -> Output {
    let module = compile(source, &path(name, "py")).expect("the program to compile");
    wasmtime(name, &module)
}

/// Asserts that the compiled `source` prints `stdout`, then stops with the
/// error of an `int` outside 31 bits at `line`, with `lines` under the
/// `File` line of the traceback.
fn stops_at_the_int_limit(
    name: &str,
    source: &str,
    stdout: &str,
    line: u32,
    lines: &str,
    result: &str,
) {
    let output = run(name, source);
    assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "stdout");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "Traceback (most recent call last):\n  \
             File \"{}\", line {line}, in <module>\n{lines}\
             spython: the result {result} does not fit in an int, \
             which goes from -1073741824 to 1073741823\n",
            path(name, "py").display()
        ),
        "stderr"
    );
    assert_eq!(output.status.code(), Some(1), "exit code");
}

fn unsupported(source: &str) -> String {
    match compile(source, Path::new("unsupported.py")) {
        Err(compiler::Error::Unsupported { what, .. }) => what,
        Err(error) => panic!("an invalid module: {error:?}"),
        Ok(_) => panic!("the program compiled"),
    }
}

#[test]
fn an_empty_program_prints_nothing() {
    same_as_cpython("an_empty_program_prints_nothing", "");
}

#[test]
fn print_writes_a_str_and_a_newline() {
    same_as_cpython("print_writes_a_str_and_a_newline", "print('olá')\n");
}

#[test]
fn print_without_arguments_writes_a_newline() {
    same_as_cpython("print_without_arguments_writes_a_newline", "print()\n");
}

#[test]
fn print_separates_its_arguments_with_a_space() {
    same_as_cpython(
        "print_separates_its_arguments_with_a_space",
        "print('a', '', 'ção', '日本', '😀')\nprint('a')\n",
    );
}

#[test]
fn print_writes_a_str_longer_than_the_buffer() {
    let text: String = "ab€😀".repeat(20_000);
    same_as_cpython(
        "print_writes_a_str_longer_than_the_buffer",
        &format!("print('{text}', '{text}')\nprint('{text}')\n"),
    );
}

#[test]
fn a_str_expression_as_a_statement_prints_nothing() {
    same_as_cpython(
        "a_str_expression_as_a_statement_prints_nothing",
        "'nada'\nprint('nada')\n'nada'\n",
    );
}

#[test]
fn a_str_with_the_escape_of_a_surrogate_is_unsupported() {
    assert_eq!(
        unsupported("print('\\ud800')\n"),
        "a str with an escape that may be a surrogate"
    );
}

#[test]
fn print_with_a_keyword_argument_is_unsupported() {
    assert_eq!(
        unsupported("print('a', end='')\n"),
        "print with keyword arguments"
    );
}

#[test]
fn print_writes_an_int() {
    same_as_cpython(
        "print_writes_an_int",
        "print(0, 7, -7, 10, 1073741823, -1073741824)\n",
    );
}

#[test]
fn the_operators_of_int_follow_python() {
    same_as_cpython(
        "the_operators_of_int_follow_python",
        "print(2 + 3 * 4, 7 - 10, -(3 - 5), +4, 2 * -3, 32768 * 32767)\n",
    );
}

#[test]
fn a_bool_is_an_int_in_an_operator() {
    same_as_cpython(
        "a_bool_is_an_int_in_an_operator",
        "print(True, False, True + True, -True, +False, True * 3)\n",
    );
}

#[test]
fn the_comparisons_of_int_give_a_bool() {
    same_as_cpython(
        "the_comparisons_of_int_give_a_bool",
        "print(1 < 2, 2 <= 1, 3 == 3, 3 != 3, 4 > 5, 5 >= 5, -1 < 0, False < True)\n\
         print(1 < 2 < 3, 1 < 3 < 2, 3 > 2 > 1 > 0, 1 == 1 != 2)\n",
    );
}

#[test]
fn a_chain_of_comparisons_stops_at_the_first_false() {
    same_as_cpython(
        "a_chain_of_comparisons_stops_at_the_first_false",
        "print(2 < 1 < 1073741823 + 1)\n",
    );
}

#[test]
fn an_int_expression_as_a_statement_prints_nothing() {
    same_as_cpython(
        "an_int_expression_as_a_statement_prints_nothing",
        "1 + 2\n2 < 3\nTrue\nprint(1)\n",
    );
}

#[test]
fn a_sum_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "a_sum_outside_31_bits_stops_the_program",
        "print('antes')\nprint(1073741823 + 1)\nprint('depois')\n",
        "antes\n",
        2,
        "    print(1073741823 + 1)\n          ~~~~~~~~~~~^~~\n",
        "1073741824",
    );
}

#[test]
fn a_difference_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "a_difference_outside_31_bits_stops_the_program",
        "print(-1073741824 - 1)\n",
        "",
        1,
        "    print(-1073741824 - 1)\n          ~~~~~~~~~~~~^~~\n",
        "-1073741825",
    );
}

#[test]
fn a_product_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "a_product_outside_31_bits_stops_the_program",
        "print(65536 * 65536)\n",
        "",
        1,
        "    print(65536 * 65536)\n          ~~~~~~^~~~~~~\n",
        "4294967296",
    );
}

#[test]
fn the_negation_of_the_least_int_stops_the_program() {
    stops_at_the_int_limit(
        "the_negation_of_the_least_int_stops_the_program",
        "print(-(-1073741823 - 1))\n",
        "",
        1,
        "    print(-(-1073741823 - 1))\n          ^^^^^^^^^^^^^^^^^^\n",
        "1073741824",
    );
}

#[test]
fn print_evaluates_every_argument_before_it_writes() {
    stops_at_the_int_limit(
        "print_evaluates_every_argument_before_it_writes",
        "print('a', 1073741823 + 1)\n",
        "",
        1,
        "    print('a', 1073741823 + 1)\n               ~~~~~~~~~~~^~~\n",
        "1073741824",
    );
}

#[test]
fn an_int_literal_outside_31_bits_is_unsupported() {
    for source in ["print(1073741824)\n", "print(-1073741825)\n"] {
        assert_eq!(unsupported(source), "an int that does not fit in 31 bits");
    }
}

#[test]
fn the_floor_division_and_the_modulo_follow_python() {
    same_as_cpython(
        "the_floor_division_and_the_modulo_follow_python",
        "print(7 // 2, -7 // 2, 7 // -2, -7 // -2, 6 // 3, -6 // 3, 0 // -5)\n\
         print(7 % 3, -7 % 3, 7 % -3, -7 % -3, 6 % 3, -6 % -3, 0 % 5)\n\
         print(-1073741824 // 1, 1073741823 // -1, -1073741824 % -1, True // True)\n",
    );
}

#[test]
fn the_floor_division_of_the_least_int_by_minus_one_stops_the_program() {
    stops_at_the_int_limit(
        "the_floor_division_of_the_least_int_by_minus_one_stops_the_program",
        "print(-1073741824 // -1)\n",
        "",
        1,
        "    print(-1073741824 // -1)\n          ~~~~~~~~~~~~^^~~~\n",
        "1073741824",
    );
}

// The checker rejects a divisor that it knows to be 0, unless a comment
// silences the error.
#[test]
fn the_floor_division_by_zero_raises_zero_division_error() {
    same_as_cpython(
        "the_floor_division_by_zero_raises_zero_division_error",
        "print(1)\nprint(7 // 0)  # ty: ignore[division-by-zero]\n",
    );
}

#[test]
fn the_modulo_by_zero_raises_zero_division_error() {
    same_as_cpython(
        "the_modulo_by_zero_raises_zero_division_error",
        "print(-7 % 0)  # ty: ignore[division-by-zero]\n",
    );
}

#[test]
fn the_true_division_is_unsupported() {
    assert_eq!(unsupported("print(7 / 2)\n"), "this operator");
}

#[test]
fn the_traceback_of_the_int_limit_shows_every_line_of_the_operator() {
    stops_at_the_int_limit(
        "the_traceback_of_the_int_limit_shows_every_line_of_the_operator",
        "print('日本', (1073741823 +\n       1))\n",
        "",
        1,
        "    print('日本', (1073741823 +\n                   ~~~~~~~~~~~^\n           1))\n           ~\n",
        "1073741824",
    );
}

#[test]
fn the_traceback_of_the_int_limit_has_no_carets_under_a_whole_line() {
    stops_at_the_int_limit(
        "the_traceback_of_the_int_limit_has_no_carets_under_a_whole_line",
        "-(-1073741823 - 1)\n",
        "",
        1,
        "    -(-1073741823 - 1)\n",
        "1073741824",
    );
}

#[test]
fn a_variable_holds_an_int_a_bool_or_a_str() {
    same_as_cpython(
        "a_variable_holds_an_int_a_bool_or_a_str",
        "n = 7\n\
         s = 'olá'\n\
         b = n > 3\n\
         print(n, s, b)\n\
         n = n * 2\n\
         s = 'mundo'\n\
         print(n, s, b, n // 4)\n\
         t: int = -3\n\
         u: str\n\
         print(t // 2, t % 2)\n",
    );
}

#[test]
fn a_chained_assignment_gives_the_value_to_every_target() {
    same_as_cpython(
        "a_chained_assignment_gives_the_value_to_every_target",
        "a = b = c = 1 + 2\nprint(a, b, c)\na = 0\nprint(a, b, c)\n",
    );
}

#[test]
fn an_augmented_assignment_follows_python() {
    same_as_cpython(
        "an_augmented_assignment_follows_python",
        "n = 10\nn += 5\nn -= 1\nn *= 3\nprint(n)\nn //= 4\nprint(n)\nn %= -4\nprint(n)\n",
    );
}

#[test]
fn an_augmented_assignment_by_zero_raises_zero_division_error() {
    same_as_cpython(
        "an_augmented_assignment_by_zero_raises_zero_division_error",
        "n = 7\nn //= 0  # ty: ignore[division-by-zero]\n",
    );
}

#[test]
fn an_augmented_assignment_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "an_augmented_assignment_outside_31_bits_stops_the_program",
        "n = 1073741823\nprint(n)\nn += 1\n",
        "1073741823\n",
        3,
        "    n += 1\n",
        "1073741824",
    );
}

// The checker rejects a read before the assignment, unless a comment
// silences the error.
#[test]
fn a_read_before_the_assignment_raises_name_error() {
    same_as_cpython(
        "a_read_before_the_assignment_raises_name_error",
        "print(1)\nprint(2, x)  # ty: ignore[unresolved-reference]\nx = 3\n",
    );
}

#[test]
fn an_augmented_assignment_before_the_assignment_raises_name_error() {
    same_as_cpython(
        "an_augmented_assignment_before_the_assignment_raises_name_error",
        "s = 1\ns += n  # ty: ignore[unresolved-reference]\nn = 3\n",
    );
}

#[test]
fn a_read_before_the_assignment_of_a_builtin_name_is_unsupported() {
    assert_eq!(
        unsupported("print(abs)  # ty: ignore[unresolved-reference]\nabs = 3\n"),
        "a name that may be a builtin"
    );
}

#[test]
fn a_variable_with_values_of_two_types_is_unsupported() {
    assert_eq!(
        unsupported("x = 1\nx = 'a'\n"),
        "a variable with values of two types"
    );
}

#[test]
fn a_variable_of_none_is_unsupported() {
    assert_eq!(unsupported("x = print(1)\n"), "a variable of this type");
}

#[test]
fn not_negates_a_bool() {
    same_as_cpython(
        "not_negates_a_bool",
        "t = True\n\
         print(not t, not not t, not 1 > 2)\n\
         n = 0\n\
         while not n == 3:\n    n += 1\n\
         print(n, not t + 1 == 2)\n",
    );
}

#[test]
fn not_takes_the_truth_of_an_int_or_a_str() {
    same_as_cpython(
        "not_takes_the_truth_of_an_int_or_a_str",
        "n = 0\ns = 'a'\n\
         print(not n, not n + 1, not -1, not s, not '', not not s)\n\
         while not n:\n    n += 1\n\
         print(n)\n",
    );
}

#[test]
fn not_of_none_is_unsupported() {
    assert_eq!(unsupported("print(not print(1))\n"), "the truth of None");
}

#[test]
fn and_and_or_combine_bools() {
    same_as_cpython(
        "and_and_or_combine_bools",
        "t = True\nf = False\n\
         print(t and t, t and f, f and t, f and f)\n\
         print(t or t, t or f, f or t, f or f)\n\
         print(t and f or t, f or t and f, not f and t, t and t and f, f or f or t)\n\
         n = 0\n\
         while n < 10 and not n == 4 or n == 7:\n    n += 1\n\
         print(n)\n",
    );
}

#[test]
fn and_and_or_skip_the_right_operand_when_the_left_decides() {
    same_as_cpython(
        "and_and_or_skip_the_right_operand_when_the_left_decides",
        "print(False and 1 // 0 == 0)  # ty: ignore[division-by-zero]\n\
         print(True or 1 // 0 == 0)  # ty: ignore[division-by-zero]\n\
         print(True and 1 % 0 == 0)  # ty: ignore[division-by-zero]\n",
    );
}

#[test]
fn and_and_or_give_an_operand_of_int_or_str() {
    same_as_cpython(
        "and_and_or_give_an_operand_of_int_or_str",
        "n = 0\nm = 5\ns = ''\nt = 'abc'\n\
         print(n and m, m and n, m and 7, n or m, m or n, n or 0)\n\
         print(s and t, t and s, t and 'x', s or t, t or s, s or '')\n\
         print(n or m and 3, n and m or -1, n or n or 9)\n\
         r = s or 'vazio'\n\
         print(r)\n\
         while m and m - 3:\n    m -= 1\n\
         print(m)\n",
    );
}

#[test]
fn and_of_operands_of_two_types_is_unsupported() {
    assert_eq!(
        unsupported("t = True\nprint(t and 1)\n"),
        "operands of two types"
    );
    assert_eq!(
        unsupported("n = 0\nprint('a' or n)\n"),
        "operands of two types"
    );
    assert_eq!(unsupported("print(print(1) or 1)\n"), "the truth of None");
}

#[test]
fn an_if_runs_the_first_clause_whose_condition_is_true() {
    same_as_cpython(
        "an_if_runs_the_first_clause_whose_condition_is_true",
        "n = 7\n\
         if n > 5:\n    print('maior')\n\
         if n < 5:\n    print('menor')\n\
         if n < 5:\n    print('a')\nelse:\n    print('b')\n\
         if n < 0:\n    s = 'negativo'\nelif n == 0:\n    s = 'zero'\nelif n % 2 == 0:\n    s = 'par'\nelse:\n    s = 'ímpar'\n\
         print(s)\n\
         if n > 0:\n    if n > 10:\n        print('grande')\n    else:\n        print('pequeno')\n    n = n * 2\n\
         print(n)\n",
    );
}

#[test]
fn a_condition_takes_the_truth_of_an_int_or_a_str() {
    same_as_cpython(
        "a_condition_takes_the_truth_of_an_int_or_a_str",
        "zero = 0\n\
         if zero:\n    print('nunca')\nelse:\n    print('zero')\n\
         if -3:\n    print('-3')\n\
         if '':\n    print('nunca')\nelif 'a':\n    print('a')\n\
         n = 3\n\
         while n:\n    print(n)\n    n -= 1\n\
         s = 'abc'\n\
         while s:\n    print(s)\n    s = ''\n\
         if n - 1:\n    print(n - 1)\n",
    );
}

#[test]
fn a_condition_of_none_is_unsupported() {
    assert_eq!(
        unsupported("if print(1):\n    print(2)\n"),
        "the truth of None"
    );
}

#[test]
fn a_while_repeats_while_the_condition_is_true() {
    same_as_cpython(
        "a_while_repeats_while_the_condition_is_true",
        "i = 0\ntotal = 0\n\
         while i < 5:\n    i += 1\n    if i == 2:\n        continue\n    total += i\n    print(i, total)\n\
         print(total)\n\
         while False:\n    print('nunca')\n",
    );
}

#[test]
fn a_pass_does_nothing() {
    same_as_cpython(
        "a_pass_does_nothing",
        "n = 0\n\
         while n < 3:\n    n += 1\n    if n == 2:\n        pass\n    else:\n        print(n)\n\
         if n == 3:\n    pass\n\
         print('fim')\n",
    );
}

#[test]
fn a_break_leaves_the_innermost_loop_without_its_else() {
    same_as_cpython(
        "a_break_leaves_the_innermost_loop_without_its_else",
        "i = 0\n\
         while True:\n    j = 0\n    while j < 10:\n        if j == 2:\n            break\n        print(i, j)\n        j += 1\n    else:\n        print('nunca')\n    i += 1\n    if i == 3:\n        break\n\
         print(i)\n\
         n = 3\n\
         while n > 0:\n    n -= 1\nelse:\n    print('fim', n)\n",
    );
}

#[test]
fn a_read_after_an_assignment_in_a_loop_finds_the_value() {
    same_as_cpython(
        "a_read_after_an_assignment_in_a_loop_finds_the_value",
        "i = 0\nwhile i < 3:\n    if i == 1:\n        x = 10\n    i += 1\n\
         print(x)  # ty: ignore[possibly-unresolved-reference]\n\
         while True:\n    y = 1\n    break\nprint(y)\n",
    );
}

#[test]
fn a_read_without_an_assignment_in_a_loop_raises_name_error() {
    same_as_cpython(
        "a_read_without_an_assignment_in_a_loop_raises_name_error",
        "i = 0\nwhile i < 3:\n    if i == 5:\n        x = 10\n    i += 1\n\
         print(i)\nprint(x)  # ty: ignore[possibly-unresolved-reference]\n",
    );
}

#[test]
fn a_division_by_a_variable_that_is_zero_raises_zero_division_error() {
    same_as_cpython(
        "a_division_by_a_variable_that_is_zero_raises_zero_division_error",
        "d = 3\nwhile d > 0:\n    print(12 // d, 12 % d)\n    d -= 1\nprint(12 // d)\n",
    );
}

#[test]
fn a_loop_that_doubles_an_int_stops_at_the_int_limit() {
    stops_at_the_int_limit(
        "a_loop_that_doubles_an_int_stops_at_the_int_limit",
        "n = 1\nwhile True:\n    n *= 2\n",
        "",
        3,
        "    n *= 2\n",
        "1073741824",
    );
}
