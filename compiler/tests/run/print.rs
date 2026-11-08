//! `print` and what a statement of an expression writes.

use std::fs::File;
use std::process::Stdio;

use indoc::indoc;

use super::{
    compile, fails_to_write, merges_as_cpython, path, same_as_cpython, stops_at_the_int_limit,
    unsupported, writes_as_cpython,
};

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

/// The output waits in the buffers, so the error comes at the exit.
#[test]
#[cfg(target_os = "linux")]
fn print_to_a_full_device_fails_at_the_exit() {
    writes_as_cpython(
        "print_to_a_full_device_fails_at_the_exit",
        indoc! {"
            def show(s: str) -> None:
                print(s, s)


            show('a')
        "},
        full_device,
    );
}

/// The `print` of 5000 bytes passes the 8192 of the `TextIOWrapper`, so
/// the text before it goes to the `BufferedWriter`, which writes it at
/// once, as it is larger than the 4096 of a block.
#[test]
#[cfg(target_os = "linux")]
fn print_raises_os_error_when_the_buffers_go_to_a_full_device() {
    writes_as_cpython(
        "print_raises_os_error_when_the_buffers_go_to_a_full_device",
        indoc! {"
            def show(n: int) -> None:
                print('x' * n)


            show(4000)
            show(95)
            show(1)
            show(5000)
            show(10000)
        "},
        full_device,
    );
}

/// A `print` with 8192 bytes or more goes to the host on its own.
#[test]
#[cfg(target_os = "linux")]
fn a_long_print_raises_os_error_on_a_full_device() {
    writes_as_cpython(
        "a_long_print_raises_os_error_on_a_full_device",
        indoc! {"
            print('a')
            print('é' * 5000)
            print('b')
        "},
        full_device,
    );
}

#[test]
#[cfg(target_os = "linux")]
fn the_exit_fails_on_a_full_device_after_a_traceback() {
    writes_as_cpython(
        "the_exit_fails_on_a_full_device_after_a_traceback",
        indoc! {"
            print('a')
            print(1 // 0)  # ty: ignore[division-by-zero]
        "},
        full_device,
    );
}

#[cfg(target_os = "linux")]
fn full_device() -> Stdio {
    Stdio::from(File::create("/dev/full").expect("/dev/full"))
}

/// wasmtime gives the module `EIO` for a pipe that the reader closed, where
/// CPython raises `BrokenPipeError`.
#[test]
fn print_to_a_closed_pipe_fails_at_the_exit() {
    fails_to_write(
        "print_to_a_closed_pipe_fails_at_the_exit",
        indoc! {"
            print('a')
        "},
        || {
            let (reader, writer) = std::io::pipe().expect("a pipe");
            drop(reader);
            Stdio::from(writer)
        },
        indoc! {"
            Exception ignored on flushing sys.stdout:
            OSError: [Errno 5] Input/output error
        "},
        120,
    );
}

/// The writes go through both buffers of CPython, in each of their paths.
#[test]
fn a_long_output_arrives_whole_and_in_order() {
    same_as_cpython(
        "a_long_output_arrives_whole_and_in_order",
        indoc! {"
            i = 0
            while i < 3000:
                print(i, 'é' * (i % 7), i * i)
                if i % 1000 == 999:
                    print('ã' * 5000)
                    print('x' * 20000, i)
                i += 1
        "},
    );
}

/// CPython flushes the standard output before it writes the traceback.
#[test]
fn the_traceback_comes_after_the_output_in_one_pipe() {
    merges_as_cpython(
        "the_traceback_comes_after_the_output_in_one_pipe",
        indoc! {"
            print('a')
            print(1 // 0)  # ty: ignore[division-by-zero]
        "},
    );
}

/// Each argument of `print` takes a temporary local, and so do an `and`,
/// a chain of comparisons, a repetition and an index. A function has at
/// most 50000 locals. A module this large takes minutes in wasmtime, so
/// the test only compiles it.
#[test]
fn many_prints_reuse_their_locals() {
    let head = indoc! {"
        s = 'ab'
        n = 1
    "};
    let args = "n, ".repeat(1000);
    let line = format!("print(n and 2, 0 < n < 2, 2 * s, s[0], {args})\n");
    let source = format!("{head}{}", line.repeat(50));
    compile(&source, &path("many_prints_reuse_their_locals", "py"))
        .expect("the program to compile");
}
