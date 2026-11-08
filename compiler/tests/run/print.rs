//! `print` and what a statement of an expression writes.

use std::fs::File;
use std::process::Stdio;

use indoc::indoc;

use super::{fails_to_write, same_as_cpython, stops_at_the_int_limit, unsupported};

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

/// `python3 -u`, which writes each `print` at once, writes the same
/// traceback.
#[test]
#[cfg(target_os = "linux")]
fn print_to_a_full_device_raises_os_error() {
    fails_to_write(
        "print_to_a_full_device_raises_os_error",
        indoc! {"
            def show(s: str) -> None:
                print(s, s)


            show('a')
        "},
        || Stdio::from(File::create("/dev/full").expect("/dev/full")),
        indoc! {r#"
            Traceback (most recent call last):
              File "{file}", line 5, in <module>
                show('a')
                ~~~~^^^^^
              File "{file}", line 2, in show
                print(s, s)
                ~~~~~^^^^^^
            OSError: [Errno 28] No space left on device
        "#},
    );
}

/// wasmtime gives the module `EIO` for a pipe that the reader closed, where
/// CPython raises `BrokenPipeError`.
#[test]
fn print_to_a_closed_pipe_raises_os_error() {
    fails_to_write(
        "print_to_a_closed_pipe_raises_os_error",
        indoc! {"
            print('a')
        "},
        || {
            let (reader, writer) = std::io::pipe().expect("a pipe");
            drop(reader);
            Stdio::from(writer)
        },
        indoc! {r#"
            Traceback (most recent call last):
              File "{file}", line 1, in <module>
                print('a')
                ~~~~~^^^^^
            OSError: [Errno 5] Input/output error
        "#},
    );
}
