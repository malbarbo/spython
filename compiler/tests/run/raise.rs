//! `raise` of a builtin exception, and `assert`.

use indoc::indoc;

use super::{same_as_cpython, unsupported};

#[test]
fn a_raise_ends_the_program_with_the_traceback_of_the_exception() {
    same_as_cpython(
        "a_raise_ends_the_program_with_the_traceback_of_the_exception",
        indoc! {"
            def check(n: int) -> int:
                if n < 0:
                    raise ValueError('negative: ' + str(n))
                return n


            def twice(n: int) -> int:
                return 2 * check(n)


            print(twice(3))
            print(twice(-3))
        "},
    );
}

#[test]
fn a_raise_without_a_text_writes_the_class_alone() {
    for (name, raise) in [
        ("class", "raise LookupError"),
        ("call", "raise RuntimeError()"),
        ("empty", "raise TypeError('')"),
        ("multi", "raise OverflowError(\n    'a' 'b'\n)"),
    ] {
        same_as_cpython(
            &format!("a_raise_without_a_text_writes_the_class_alone_{name}"),
            &format!("print('before')\n{raise}\n"),
        );
    }
}

#[test]
fn a_raise_names_each_class_as_cpython_does() {
    for class in [
        "OSError",
        "IOError",
        "EnvironmentError",
        "TimeoutError",
        "StopIteration",
        "AttributeError",
        "NameError",
        "ImportError",
        "UserWarning",
        "BaseException",
        "GeneratorExit",
    ] {
        same_as_cpython(
            &format!("a_raise_names_each_class_as_cpython_does_{class}"),
            &format!("raise {class}('the text: ã')\n"),
        );
    }
}

#[test]
fn a_raise_evaluates_its_argument_before_it_raises() {
    same_as_cpython(
        "a_raise_evaluates_its_argument_before_it_raises",
        indoc! {"
            def text(n: int) -> str:
                print('text', n)
                return str(10 // n)


            def f(n: int) -> int:
                raise ValueError(text(n))


            def g(n: int) -> str:
                while True:
                    if n > 0:
                        raise IndexError
                    n += 1


            print('skip')
            f(0)
        "},
    );
}

#[test]
fn each_raise_outside_what_the_compiler_covers_has_its_own_message() {
    assert_eq!(
        unsupported("raise KeyError('a')\n"),
        "a raise of this exception"
    );
    assert_eq!(
        unsupported(indoc! {"
            class E(Exception):
                pass

            raise E('a')
        "}),
        "a class other than an Enum"
    );
    assert_eq!(
        unsupported("raise ValueError(1)\n"),
        "an exception with this argument"
    );
    assert_eq!(
        unsupported("raise ValueError('a', 'b')\n"),
        "a raise with these arguments"
    );
    assert_eq!(
        unsupported("raise ValueError('a') from None\n"),
        "this raise"
    );
    assert_eq!(
        unsupported(indoc! {"
            def f() -> None:
                raise
        "}),
        "this raise"
    );
}

#[test]
fn an_assert_that_holds_does_nothing() {
    same_as_cpython(
        "an_assert_that_holds_does_nothing",
        indoc! {"
            def text() -> str:
                print('text')
                return 'no'


            def f(n: int) -> int:
                assert n > 0
                assert n, text()
                assert 'a' and n < 100, 'large'
                return n


            print(f(3))
            assert f(4) == 4
        "},
    );
}

#[test]
fn an_assert_that_fails_raises_assertion_error_at_the_condition() {
    for (name, source) in [
        ("bare", "x = 1\nassert x > 1\n"),
        ("text", "x = 1\nassert x > 1, 'x is ' + str(x)\n"),
        ("empty", "x = 1\nassert x > 1, ''\n"),
        ("whole", "x = ''\nassert x\n"),
        ("lines", "x = 1\nassert (\n    x >\n    1\n), 'two lines'\n"),
        (
            "function",
            "def f(n: int) -> int:\n    assert n > 0, 'neg'\n    return n\n\n\nprint(f(1))\nprint(f(-1))\n",
        ),
        (
            "raises",
            "def text() -> str:\n    return str(1 // int(0.0))\n\n\nassert 1 > 2, text()\n",
        ),
    ] {
        same_as_cpython(
            &format!("an_assert_that_fails_raises_assertion_error_at_the_condition_{name}"),
            source,
        );
    }
}

#[test]
fn an_assert_with_a_text_that_is_not_a_str_is_unsupported() {
    assert_eq!(
        unsupported("x = 1\nassert x > 1, x\n"),
        "an exception with this argument"
    );
}
