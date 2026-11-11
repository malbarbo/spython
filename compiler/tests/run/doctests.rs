//! The doctests of a file, compared with the runner of spython, which runs
//! here in CPython.

use std::path::Path;
use std::process::{Command, Output};

use engine::Level;
use engine::check::check_source;
use indoc::indoc;

use super::{deno, output_within_limit, path, python_program, wasmtime};

/// The runner of the `test` command of spython.
const RUNNER: &str = include_str!("../../../engine/src/doctest_runner.py");

/// The module that runs the doctests of `source`, which has to pass the
/// checker at the last level. A traceback names the file by `path`, and
/// the report has the examples that pass when `verbose` is `true`.
pub(super) fn compile_tests(
    source: &str,
    path: &Path,
    verbose: bool,
) -> Result<compiler::Module, compiler::Error> {
    let checked = match check_source(source, Level::Full, false) {
        Ok(checked) => checked,
        Err(errors) => panic!("the checker rejects the program: {:?}", errors.diagnostics),
    };
    let program = checked.into_doctests();
    let docstrings = docstrings(program.docstrings);
    let path = path.to_str().expect("a path in utf-8");
    compiler::compile_tests(
        &program.db,
        program.file,
        path,
        &docstrings,
        &program.copies,
        verbose,
    )
}

/// The docstrings of the checker as the compiler takes them.
pub(super) fn docstrings(found: Vec<engine::check::DoctestFunction>) -> Vec<compiler::Docstring> {
    found
        .into_iter()
        .map(|docstring| compiler::Docstring {
            owner: docstring.owner,
            function: docstring.name,
            examples: docstring
                .examples
                .into_iter()
                .map(|(line, example)| compiler::Example {
                    line,
                    source: example.source,
                    want: example.want,
                    exc_msg: example.exc_msg,
                })
                .collect(),
        })
        .collect()
}

/// `text` as a literal of Python, with an escape for each character that is
/// not plain ASCII.
fn py_literal(text: &str) -> String {
    let mut literal = String::from("\"");
    for c in text.chars() {
        if (' '..='~').contains(&c) && c != '\\' && c != '"' {
            literal.push(c);
        } else {
            literal.push_str(&format!("\\U{:08x}", u32::from(c)));
        }
    }
    literal.push('"');
    literal
}

/// Runs the doctests of the file at `file`, which holds `source`, with the
/// runner of spython in CPython. The runner asks the checker for the
/// docstrings of the file, and gets them here from the extraction of the
/// checker.
///
/// The runner writes the traceback of a file that raises without its own
/// frame, which the interpreter of spython leaves out. CPython 3.13 writes
/// the frame, and the output here does not have it.
pub(super) fn runner(name: &str, file: &Path, source: &str, verbose: bool) -> Output {
    let docstrings: String = engine::doctests::docstrings(source)
        .expect("a source that parses")
        .iter()
        .map(|(owner, doc)| format!("    [{}, {}],\n", py_literal(owner), py_literal(doc)))
        .collect();
    let driver = format!(
        "import sys, types\n\
         _ffi = types.ModuleType('_spython_ffi')\n\
         _ffi.doctest_docstrings = lambda source: [\n{docstrings}]\n\
         _ffi.keep_source = lambda path, source: None\n\
         sys.modules['_spython_ffi'] = _ffi\n\
         {RUNNER}\n\
         sys.exit(_main(sys.argv[1:]))\n"
    );
    let driver_path = path(name, "runner.py");
    std::fs::write(&driver_path, driver).expect("the driver written to disk");
    let mut command = Command::new(python_program());
    command.arg(&driver_path);
    if verbose {
        command.arg("--verbose");
    }
    command
        .arg(file)
        .arg(file.parent().expect("a file in a directory"));
    let mut output = output_within_limit(command);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let frame = format!("  File \"{}\", line ", driver_path.display());
    let mut skip = 0;
    let lines: Vec<&str> = stderr
        .split_inclusive('\n')
        .filter(|line| {
            // The frame has the line of the file, the line of the code and
            // the line of the marks.
            if line.starts_with(&frame) {
                skip = 3;
            }
            let keep = skip == 0;
            skip = usize::saturating_sub(skip, 1);
            keep
        })
        .collect();
    output.stderr = lines.concat().into_bytes();
    output
}

/// Asserts that the module of the doctests of `source` writes what the
/// runner writes, and exits with 1 when the runner counts a failure, in
/// wasmtime and in deno.
pub(super) fn same_as_the_runner(name: &str, source: &str) {
    same_as_the_runner_with(name, source, false);
}

/// Asserts what [`same_as_the_runner`] asserts, for a report that has the
/// examples that pass when `verbose` is `true`.
fn same_as_the_runner_with(name: &str, source: &str, verbose: bool) {
    let file = path(name, "py");
    std::fs::write(&file, source).expect("the program written to disk");
    let module = compile_tests(source, &file, verbose).expect("the program to compile");
    let ours = wasmtime(name, &module, None);
    assert_eq!(deno(name), ours, "deno and wasmtime");
    let theirs = runner(name, &file, source, verbose);
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
    assert_eq!(
        ours.status.code(),
        theirs.status.code().map(|code| code.min(1)),
        "exit code"
    );
}

#[test]
fn the_examples_that_pass_give_the_summary_alone() {
    same_as_the_runner(
        "the_examples_that_pass_give_the_summary_alone",
        indoc! {r#"
            """
            >>> dobro(2)
            4
            """


            def dobro(n: int) -> int:
                """
                O dobro de n.

                >>> dobro(3)
                6
                >>> dobro(-1) + 1
                -1
                """
                return 2 * n


            def nome(s: str) -> str:
                """
                >>> nome('ana')
                'Ana?'
                >>> print(nome('b'))
                B?
                >>> nome('') == '?'
                True
                >>> 1 / 4
                0.25
                """
                if s == '':
                    return '?'
                return maiuscula(s[0]) + sem_primeira(s) + '?'


            def maiuscula(c: str) -> str:
                if c == 'a':
                    return 'A'
                if c == 'b':
                    return 'B'
                return c


            def sem_primeira(s: str) -> str:
                r = ''
                i = 1
                while i < len(s):
                    r = r + s[i]
                    i = i + 1
                return r


            print('the module runs first')
        "#},
    );
}

#[test]
fn an_example_that_fails_shows_what_it_expected_and_what_it_got() {
    same_as_the_runner(
        "an_example_that_fails_shows_what_it_expected_and_what_it_got",
        indoc! {r#"
            def dobro(n: int) -> int:
                """
                >>> dobro(3)
                7
                >>> dobro(1)
                2
                >>> print(dobro(1), 'x')
                2 y
                >>> print('a')
                >>> dobro(2)
                >>> x = dobro(5)
                10
                >>> if dobro(1) == 2:
                ...     dobro(4)
                ...     print('yes')
                8
                no
                """
                return 2 * n


            def texto() -> str:
                """
                >>> texto()
                'a b'
                >>> texto()
                "a'b"
                """
                return "a'b"
        "#},
    );
}

#[test]
fn an_example_that_raises_is_an_error_unless_it_expects_the_exception() {
    same_as_the_runner(
        "an_example_that_raises_is_an_error_unless_it_expects_the_exception",
        indoc! {r#"
            def inverso(n: int) -> float:
                """
                >>> inverso(4)
                0.25
                >>> inverso(0)
                0.0
                >>> inverso(0)
                Traceback (most recent call last):
                    ...
                ZeroDivisionError: division by zero
                >>> inverso(0)
                Traceback (most recent call last):
                ValueError: other
                >>> inverso(2)
                Traceback (most recent call last):
                ZeroDivisionError: division by zero
                >>> print('before'); inverso(0)
                before
                ZeroDivisionError: division by zero
                >>> x = inverso(0)
                >>> x
                1.0
                >>> y = 3
                >>> y
                3
                """
                return 1 / n


            def checa(n: int) -> int:
                """
                >>> checa(-1)
                Traceback (most recent call last):
                ValueError
                >>> checa(-2)
                Traceback (most recent call last):
                ValueError: negativo: -2
                >>> checa(1)
                1
                """
                if n == -1:
                    raise ValueError
                assert n > 0, 'negativo: ' + str(n)
                return n
        "#},
    );
}

/// An example writes the `repr` of a list, and one that differs fails.
#[test]
fn an_example_writes_the_repr_of_a_list() {
    same_as_the_runner(
        "an_example_writes_the_repr_of_a_list",
        indoc! {r#"
            def dobra(xs: list[int]) -> list[int]:
                """
                >>> dobra([1, 2])
                [1, 2, 1, 2]
                >>> dobra([])
                []
                >>> dobra([3])
                [3]
                """
                ys: list[int] = []
                i = 0
                while i < 2:
                    j = 0
                    while j < len(xs):
                        ys.append(len(xs) + j)
                        j += 1
                    i += 1
                return ys
        "#},
    );
}

/// An example writes the `repr` of a member, and compares members.
#[test]
fn an_example_writes_the_repr_of_a_member_of_an_enum() {
    same_as_the_runner(
        "an_example_writes_the_repr_of_a_member_of_an_enum",
        indoc! {r#"
            from enum import Enum, auto

            class Cor(Enum):
                VERDE = auto()
                AZUL = auto()

            def troca(c: Cor) -> Cor:
                """
                >>> troca(Cor.VERDE)
                <Cor.AZUL: 2>
                >>> troca(Cor.AZUL).name
                'VERDE'
                >>> troca(Cor.AZUL) == Cor.VERDE
                True
                >>> troca(Cor.AZUL)
                <Cor.AZUL: 2>
                """
                if c == Cor.VERDE:
                    return Cor.AZUL
                return Cor.VERDE
        "#},
    );
}

/// An example writes the `repr` of an object of a dataclass.
#[test]
fn an_example_writes_the_repr_of_a_dataclass() {
    same_as_the_runner(
        "an_example_writes_the_repr_of_a_dataclass",
        indoc! {r#"
            from dataclasses import dataclass

            @dataclass
            class Tempo:
                horas: int
                minutos: int

            def tempo(minutos: int) -> Tempo:
                """
                >>> tempo(130)
                Tempo(horas=2, minutos=10)
                >>> tempo(10)
                Tempo(horas=0, minutos=1)
                >>> tempo(10).minutos
                10
                """
                return Tempo(minutos // 60, minutos % 60)
        "#},
    );
}

/// An exception inside a `repr` leaves no object among the objects whose
/// `repr` runs, as the `finally` of `reprlib` in CPython.
#[test]
fn an_example_after_an_exception_in_a_repr_writes_the_whole_repr() {
    same_as_the_runner(
        "an_example_after_an_exception_in_a_repr_writes_the_whole_repr",
        indoc! {r#"
            from dataclasses import dataclass

            @dataclass
            class No:
                v: int

            t = No(0)

            def f() -> str:
                """
                >>> f()
                Traceback (most recent call last):
                  ...
                RecursionError: maximum recursion depth exceeded
                >>> t
                No(v=0)
                """
                return repr(t) + f()
        "#},
    );
}

#[test]
fn the_output_of_an_example_may_have_blank_lines_and_code_points_beyond_ascii() {
    same_as_the_runner(
        "the_output_of_an_example_may_have_blank_lines_and_code_points_beyond_ascii",
        indoc! {r#"
            def linhas(s: str) -> None:
                """
                >>> linhas('a')
                a
                <BLANKLINE>
                a
                >>> linhas('b')
                b
                <BLANKLINE>
                c
                >>> linhas('ação')
                ação
                <BLANKLINE>
                ação
                >>> linhas('é')
                e
                <BLANKLINE>
                é
                >>> print('  ')
                <BLANKLINE>
                >>> print('x  ')
                x
                >>> print('  ', 'y')
                y
                >>> 'ação' + '日本'
                'ação日本'
                >>> print('<BLANKLINE>x')
                <BLANKLINE>x
                >>> print('<BLANKLINE>x')
                <BLANKLINE>
                >>> print()
                """
                print(s)
                print()
                print(s)
        "#},
    );
}

#[test]
fn the_examples_of_a_docstring_share_their_names_and_copy_the_names_of_the_module() {
    same_as_the_runner(
        "the_examples_of_a_docstring_share_their_names_and_copy_the_names_of_the_module",
        indoc! {r#"
            """
            >>> total
            10
            >>> total = total + 1
            >>> total
            11
            >>> soma(total)
            21
            """

            total = 10


            def soma(n: int) -> int:
                """
                >>> a = 2
                >>> b = soma(a)
                >>> a + b
                14
                >>> total
                10
                """
                return n + total


            def zeta() -> None:
                """
                >>> zeta()
                z 10
                """
                print('z', total)


            def alfa() -> None:
                """No example here."""


            print('total', total)
        "#},
    );
}

#[test]
fn an_example_that_assigns_a_name_of_the_module_without_a_value_starts_without_it() {
    same_as_the_runner(
        "an_example_that_assigns_a_name_of_the_module_without_a_value_starts_without_it",
        indoc! {r#"
            def zero() -> int:
                return 0


            if zero() > 0:
                x = 1
                s = 'a'


            def f() -> None:
                """
                >>> x = 2
                >>> x
                2
                >>> print(s)
                >>> s = 'b'
                >>> s
                'b'
                """
        "#},
    );
}

#[test]
fn a_file_without_an_example_writes_nothing_of_the_tests() {
    same_as_the_runner(
        "a_file_without_an_example_writes_nothing_of_the_tests",
        indoc! {r#"
            def f() -> None:
                """Nothing to run."""


            print('only the module')
        "#},
    );
}

#[test]
fn a_file_that_raises_before_its_tests_says_so() {
    same_as_the_runner(
        "a_file_that_raises_before_its_tests_says_so",
        indoc! {r#"
            def inverso(n: int) -> float:
                """
                >>> inverso(4)
                0.25
                """
                return 1 / n


            print('before')
            print(inverso(0))
        "#},
    );
}

#[test]
fn an_example_that_leaves_the_limit_of_an_int_stops_the_program() {
    let name = "an_example_that_leaves_the_limit_of_an_int_stops_the_program";
    let source = indoc! {r#"
        def sucessor(n: int) -> int:
            """
            >>> sucessor(1)
            3
            >>> if sucessor(0) == 1:
            ...     print('before')
            ...     sucessor(1073741823)
            0
            >>> sucessor(2)
            3
            """
            return n + 1
    "#};
    let file = path(name, "py");
    let module = compile_tests(source, &file, false).expect("the program to compile");
    let output = wasmtime(name, &module, None);
    assert_eq!(deno(name), output, "deno and wasmtime");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "before\n",
        "stdout"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            indoc! {r#"
                Running tests...
                  sucessor
                    Failed example:
                      sucessor(1)
                    Expected:
                      3
                    Got:
                      2
                Traceback (most recent call last):
                  File "<doctest sucessor[1]>", line 3, in <module>
                    sucessor(1073741823)
                    ~~~~~~~~^^^^^^^^^^^^
                  File "{file}", line 12, in sucessor
                    return n + 1
                           ~~^~~
                spython: the result 1073741824 does not fit in an int, which goes from -1073741824 to 1073741823
            "#},
            file = file.display()
        ),
        "stderr"
    );
    assert_eq!(output.status.code(), Some(1), "exit code");
}

#[test]
fn a_return_in_an_example_is_unsupported() {
    let source = indoc! {r#"
        def f() -> None:
            """
            >>> return
            """
    "#};
    match compile_tests(source, Path::new("unsupported.py"), false) {
        Err(compiler::Error::Unsupported { what, .. }) => {
            assert_eq!(what, "a return outside a function");
        }
        _ => panic!("the program compiled"),
    }
}

#[test]
fn an_example_whose_value_may_be_none_is_unsupported() {
    let source = indoc! {r#"
        def f(s: str | None) -> str | None:
            """
            >>> f(None)
            """
            return s
    "#};
    match compile_tests(source, Path::new("unsupported.py"), false) {
        Err(compiler::Error::Unsupported { what, .. }) => {
            assert_eq!(what, "an example whose value may be None");
        }
        _ => panic!("the program compiled"),
    }
}

#[test]
fn an_example_whose_value_may_be_an_object_or_none_is_unsupported() {
    let source = indoc! {r#"
        from dataclasses import dataclass


        @dataclass
        class Ponto:
            x: int


        def acha(pontos: list[Ponto]) -> Ponto | None:
            """
            >>> acha([])
            """
            if len(pontos) == 0:
                return None
            return pontos[0]
    "#};
    match compile_tests(source, Path::new("unsupported.py"), false) {
        Err(compiler::Error::Unsupported { what, .. }) => {
            assert_eq!(what, "an example whose value may be None");
        }
        _ => panic!("the program compiled"),
    }
}

#[test]
fn an_example_writes_a_value_of_a_union_without_none() {
    same_as_the_runner(
        "an_example_writes_a_value_of_a_union_without_none",
        indoc! {r#"
            def f(n: int) -> int | str:
                """
                >>> f(1)
                1
                >>> f(0)
                'zero'
                """
                if n == 0:
                    return 'zero'
                return n
        "#},
    );
}

#[test]
fn a_verbose_report_has_the_first_line_of_each_example_that_passes() {
    same_as_the_runner_with(
        "a_verbose_report_has_the_first_line_of_each_example_that_passes",
        indoc! {r#"
            def dobro(n: int) -> int:
                """
                >>> dobro(3)
                6
                >>> if dobro(1) == 2:
                ...     print('yes')
                yes
                >>> dobro(2)
                5
                """
                return 2 * n
        "#},
        true,
    );
}

#[test]
fn an_example_assigns_a_name_that_the_module_assigns_in_a_dead_branch() {
    same_as_the_runner(
        "an_example_assigns_a_name_that_the_module_assigns_in_a_dead_branch",
        indoc! {r#"
            n = 0
            if n > 0:
                s = 'a'


            def f() -> None:
                """
                >>> s = 'b'
                >>> s
                'b'
                """
        "#},
    );
}

#[test]
fn an_example_assigns_a_name_that_the_checker_narrows_in_the_module() {
    same_as_the_runner(
        "an_example_assigns_a_name_that_the_checker_narrows_in_the_module",
        indoc! {r#"
            x: str | None = None
            x = 'a'


            def f() -> None:
                """
                >>> x
                'a'
                >>> x = 'b'
                >>> x
                'b'
                """
        "#},
    );
}
