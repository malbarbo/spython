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
/// `WASMTIME` names. The stack of 8 MiB holds the 1000 frames of the
/// recursion limit of CPython.
fn wasmtime(name: &str, module: &[u8]) -> Output {
    let path = path(name, "wasm");
    std::fs::write(&path, module).expect("the module written to disk");
    let wasmtime = std::env::var("WASMTIME").unwrap_or_else(|_| "wasmtime".to_owned());
    let mut command = Command::new(wasmtime);
    command
        .args([
            "run",
            "-W",
            "gc=y,function-references=y,exceptions=y,max-wasm-stack=8388608",
        ])
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
fn the_product_of_a_str_and_an_int_repeats_the_str() {
    same_as_cpython(
        "the_product_of_a_str_and_an_int_repeats_the_str",
        "n = 3\nprint('ab' * n, n * 'é', 'ab' * 0, 'ab' * -2, '' * 5, 'x' * True, False * 'x')\ns = 'ab'\ns *= 5\nprint(s)\nprint('xyz' * 7)\n",
    );
}

#[test]
fn the_product_of_an_int_and_a_str_evaluates_the_int_first() {
    same_as_cpython(
        "the_product_of_an_int_and_a_str_evaluates_the_int_first",
        "def f(s: str) -> str:\n    print(s)\n    return s\n\n\ndef g(n: int) -> int:\n    print(n)\n    return n\n\n\nprint(g(2) * f('a'))\nprint(f('b') * g(3))\n",
    );
}

#[test]
fn a_str_longer_than_an_array_raises_memory_error() {
    let output = run(
        "a_str_longer_than_an_array_raises_memory_error",
        "s = 'abc'\nprint(1)\nprint(s * 1073741823)\n",
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n", "stdout");
    let path = path("a_str_longer_than_an_array_raises_memory_error", "py");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "Traceback (most recent call last):\n  File \"{}\", line 3, in <module>\n    print(s * 1073741823)\n          ~~^~~~~~~~~~~~\nMemoryError\n",
            path.display()
        ),
        "stderr"
    );
    assert_eq!(output.status.code(), Some(1), "exit code");
}

#[test]
fn len_gives_the_number_of_code_points_of_a_str() {
    same_as_cpython(
        "len_gives_the_number_of_code_points_of_a_str",
        "s = 'aé\u{1f600}'\nprint(len(s), len(''), len(s * 100) + 1, len('ab' + s) == 5)\n",
    );
}

#[test]
fn len_of_a_bool_or_an_int_is_unsupported() {
    assert_eq!(
        unsupported("print(len(1))  # ty: ignore[invalid-argument-type]\n"),
        "len of this type"
    );
}

#[test]
fn an_index_gives_the_code_point_of_a_str() {
    same_as_cpython(
        "an_index_gives_the_code_point_of_a_str",
        "s = 'aé\u{1f600}z'\nn = 2\nprint(s[0], s[1], s[n], s[3], s[-1], s[-4], s[True], s[0][0][-1])\nprint(s[0] + s[0] == 'aa', s[2] * 2, len(s[2]))\n",
    );
}

#[test]
fn an_index_evaluates_the_str_first() {
    same_as_cpython(
        "an_index_evaluates_the_str_first",
        "def f(s: str) -> str:\n    print(s)\n    return s\n\n\ndef g(n: int) -> int:\n    print(n)\n    return n\n\n\nprint(f('ab')[g(1)])\n",
    );
}

#[test]
fn an_index_out_of_range_raises_index_error() {
    same_as_cpython(
        "an_index_out_of_range_raises_index_error",
        "def at(s: str, n: int) -> str:\n    return s[n]\n\n\nprint(at('abc', -3))\nprint(at('abc', 3))\n",
    );
    same_as_cpython(
        "an_index_out_of_range_raises_index_error_from_the_end",
        "def at(s: str, n: int) -> str:\n    return s[n]\n\n\nprint(at('abc', -4))\n",
    );
    same_as_cpython(
        "an_index_of_the_empty_str_raises_index_error",
        "def at(s: str, n: int) -> str:\n    return s[n]\n\n\nprint(at('', 0))\n",
    );
}

#[test]
fn a_slice_is_unsupported() {
    assert_eq!(unsupported("print('abc'[1:2])\n"), "a slice");
}

#[test]
fn the_comparisons_of_str_follow_the_order_of_the_code_points() {
    let strs = [
        "",
        "a",
        "b",
        "ab",
        "aa",
        "B",
        "é",
        "\u{1f600}",
        "a\u{1f600}",
    ];
    let mut source = String::new();
    for a in strs {
        for b in strs {
            source.push_str(&format!(
                "print('{a}' == '{b}', '{a}' != '{b}', '{a}' < '{b}', '{a}' <= '{b}', '{a}' > '{b}', '{a}' >= '{b}')\n"
            ));
        }
    }
    same_as_cpython(
        "the_comparisons_of_str_follow_the_order_of_the_code_points",
        &source,
    );
}

#[test]
fn a_chain_of_comparisons_of_str_stops_at_the_first_false() {
    same_as_cpython(
        "a_chain_of_comparisons_of_str_stops_at_the_first_false",
        "def f() -> str:\n    print('f')\n    return 'z'\n\n\na = 'a'\nprint(a < 'b' < 'c', a < 'c' < 'b', 'b' < a < f(), a < 'b' < f(), a == a != 'b')\n",
    );
}

#[test]
fn a_comparison_of_a_str_and_an_int_is_unsupported() {
    assert_eq!(
        unsupported("print('a' == 1)\n"),
        "a comparison of two types"
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
fn the_sum_of_two_strs_joins_them() {
    same_as_cpython(
        "the_sum_of_two_strs_joins_them",
        "s = 'ab' + 'çd'\nprint(s + '' + s)\nprint('' + '')\nt = 'x'\nt += s\nt += t\nprint(t)\n",
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
fn and_and_or_of_two_types_take_the_truth_of_each_where_only_the_truth_matters() {
    same_as_cpython(
        "and_and_or_of_two_types_take_the_truth_of_each_where_only_the_truth_matters",
        "m = 3\nt = 'abc'\n\
         while m and t:\n    m -= 1\n\
         print(m)\n\
         if not (m or t) or m and not t:\n    print('nunca')\nelif t and (m == 0 or t):\n    print('sim')\n\
         print(not (m and t), not ('' or 2), not (m or t and 1 > 2))\n\
         print(not (0 and 1 // 0))  # ty: ignore[division-by-zero]\n",
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

#[test]
fn a_function_takes_its_arguments_and_returns_its_result() {
    same_as_cpython(
        "a_function_takes_its_arguments_and_returns_its_result",
        "def soma(a: int, b: int) -> int:\n    total = a + b\n    return total\n\
         \n\
         def maior(a: int, b: int) -> int:\n    if a > b:\n        return a\n    else:\n        return b\n\
         \n\
         def eco(s: str, vezes: int) -> None:\n    while vezes > 0:\n        print(s)\n        vezes -= 1\n\
         \n\
         def primeiro(n: int) -> int:\n    i = 2\n    while True:\n        if n % i == 0:\n            return i\n        i += 1\n\
         \n\
         def nada(b: bool) -> None:\n    if b:\n        return\n    print('nada')\n\
         \n\
         def par(n: int) -> bool:\n    return n % 2 == 0\n\
         \n\
         def texto(b: bool) -> str:\n    if b:\n        return 'sim'\n    return 'não'\n\
         \n\
         print(soma(2, 3), maior(soma(1, 1), 7), maior(9, 4))\n\
         eco('oi', 2)\n\
         print(primeiro(91), par(4), texto(par(3)))\n\
         nada(True)\n\
         nada(False)\n",
    );
}

#[test]
fn a_function_assigns_a_global_that_it_declares() {
    same_as_cpython(
        "a_function_assigns_a_global_that_it_declares",
        "count = 0\nname = 'a'\n\n\ndef bump(n: int) -> int:\n    global count, name\n    count += n\n    if count > 2:\n        name = name + 'b'\n    return count\n\n\ndef local() -> None:\n    count = 10\n    print(count)\n\n\nprint(bump(1), bump(2), count, name)\nlocal()\nprint(count)\n",
    );
}

#[test]
fn a_global_that_only_a_function_assigns_has_a_value_after_the_call() {
    same_as_cpython(
        "a_global_that_only_a_function_assigns_has_a_value_after_the_call",
        "name: str\n\n\ndef init() -> None:\n    global name\n    name = 'ana'\n\n\ndef show() -> None:\n    print(name)\n\n\ninit()\nshow()\nprint(name)  # ty: ignore[possibly-unresolved-reference]\n",
    );
}

#[test]
fn a_global_read_before_its_assignment_raises_name_error() {
    same_as_cpython(
        "a_global_read_before_its_assignment_raises_name_error",
        "def bump() -> None:\n    global total\n    total += 1\n\n\nbump()\ntotal = 0\n",
    );
}

#[test]
fn a_global_with_values_of_two_types_is_unsupported() {
    assert_eq!(
        unsupported(
            "x = 1\n\n\ndef f() -> None:\n    global x\n    x = 'a'  # ty: ignore[invalid-assignment]\n"
        ),
        "a variable with values of two types"
    );
}

#[test]
fn a_function_reads_the_globals_and_calls_a_function_defined_after_it() {
    same_as_cpython(
        "a_function_reads_the_globals_and_calls_a_function_defined_after_it",
        "base = 10\n\
         \n\
         def f(n: int) -> int:\n    return g(n) + base\n\
         \n\
         def g(n: int) -> int:\n    return n * fator\n\
         \n\
         fator = 3\n\
         print(f(2))\n\
         base = 100\n\
         print(f(2))\n",
    );
}

#[test]
fn an_exception_in_a_function_has_the_frames_of_the_calls() {
    same_as_cpython(
        "an_exception_in_a_function_has_the_frames_of_the_calls",
        "def k(n: int) -> int:\n    return 1 // n\n\
         \n\
         def g(n: int) -> int:\n    return k(n) + 1\n\
         \n\
         print(g(1))\n\
         print(k(2) + g(0))\n",
    );
}

#[test]
fn a_read_of_a_local_before_its_assignment_raises_unbound_local_error() {
    same_as_cpython(
        "a_read_of_a_local_before_its_assignment_raises_unbound_local_error",
        "def f(a: int) -> int:\n    if a > 0:\n        x = 1\n    return x + 7 // a  # ty: ignore[possibly-unresolved-reference]\n\
         \n\
         def g(n: int) -> int:\n    return f(n) + 1\n\
         \n\
         print(g(1))\n\
         print(g(0))\n",
    );
}

#[test]
fn a_read_of_a_global_before_its_assignment_raises_name_error() {
    same_as_cpython(
        "a_read_of_a_global_before_its_assignment_raises_name_error",
        "def h() -> int:\n    return y\n\
         \n\
         print(h())\n\
         y = 3\n",
    );
}

#[test]
fn a_call_before_the_def_raises_name_error() {
    same_as_cpython(
        "a_call_before_the_def_raises_name_error",
        "def g() -> int:\n    return f() + 1\n\
         \n\
         print('antes')\n\
         print(g())\n\
         \n\
         def f() -> int:\n    return 1\n",
    );
}

#[test]
fn a_call_before_the_def_in_the_module_raises_name_error() {
    same_as_cpython(
        "a_call_before_the_def_in_the_module_raises_name_error",
        "print(f())  # ty: ignore[unresolved-reference]\n\
         \n\
         def f() -> int:\n    return 1\n",
    );
}

#[test]
fn the_name_error_of_a_name_of_a_module_asks_about_an_import() {
    same_as_cpython(
        "the_name_error_of_a_name_of_a_module_asks_about_an_import",
        "def h() -> str:\n    return string\n\
         \n\
         print(h())\n\
         string = 'a'\n",
    );
    same_as_cpython(
        "the_name_error_of_a_call_of_a_name_of_a_module_asks_about_an_import",
        "print(random())  # ty: ignore[unresolved-reference]\n\
         \n\
         def random() -> int:\n    return 4\n",
    );
}

#[test]
fn a_recursive_function_runs_up_to_the_limit_of_cpython() {
    same_as_cpython(
        "a_recursive_function_runs_up_to_the_limit_of_cpython",
        "def count(n: int) -> int:\n    if n == 0:\n        return 0\n    return 1 + count(n - 1)\n\
         \n\
         print(count(998))\n\
         print(count(999))\n",
    );
}

#[test]
fn a_recursion_without_end_raises_recursion_error() {
    same_as_cpython(
        "a_recursion_without_end_raises_recursion_error",
        "def f(n: int) -> int:\n    return f(n + 1)\n\nprint(f(0))\n",
    );
}

#[test]
fn the_traceback_writes_how_many_frames_on_the_same_line_it_leaves_out() {
    for (name, n) in [
        ("the_traceback_leaves_out_no_frame", 2),
        ("the_traceback_leaves_out_one_frame", 4),
        ("the_traceback_leaves_out_two_frames", 5),
    ] {
        same_as_cpython(
            name,
            &format!(
                "def f(n: int) -> int:\n    if n == 0:\n        return 1 // n  # ty: ignore[division-by-zero]\n    return f(n - 1)\n\
                 \n\
                 print(f({n}))\n"
            ),
        );
    }
}

#[test]
fn a_mutual_recursion_has_every_frame_in_the_traceback() {
    same_as_cpython(
        "a_mutual_recursion_has_every_frame_in_the_traceback",
        "def a(n: int) -> int:\n    return b(n)\n\
         \n\
         def b(n: int) -> int:\n    if n == 0:\n        return 1 // n  # ty: ignore[division-by-zero]\n    return a(n - 1)\n\
         \n\
         print(a(3))\n",
    );
}

#[test]
fn a_function_that_may_end_without_a_return_is_unsupported() {
    assert_eq!(
        unsupported(
            "def f(n: int) -> int:  # ty: ignore[invalid-return-type]\n    if n > 0:\n        return 1\n"
        ),
        "a function that may end without a return"
    );
    assert_eq!(
        unsupported(
            "def f(n: int) -> int:  # ty: ignore[invalid-return-type]\n    while True:\n        if n > 0:\n            break\n        return 1\n"
        ),
        "a function that may end without a return"
    );
}

#[test]
fn a_def_outside_what_the_compiler_covers_is_unsupported() {
    assert_eq!(
        unsupported("def f(n: int = 1) -> int:\n    return n\n"),
        "a parameter with a default"
    );
    assert_eq!(
        unsupported("def f(n: int) -> int:\n    return n\n\nprint(f)\n"),
        "a function as a value"
    );
    assert_eq!(
        unsupported("def f() -> None:\n    def g() -> None:\n        pass\n"),
        "a def in a function"
    );
    assert_eq!(
        unsupported(
            "def f() -> int:\n    return 1\n\nprint(f(n=1))  # ty: ignore[unknown-argument]\n"
        ),
        "a call with keyword arguments"
    );
}
