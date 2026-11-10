//! The values of a union of types.

use indoc::indoc;

use super::{same_as_cpython, unsupported};

#[test]
fn a_str_or_none_goes_through_variables_parameters_and_results() {
    same_as_cpython(
        "a_str_or_none_goes_through_variables_parameters_and_results",
        indoc! {"
            def pick(flag: bool) -> str | None:
                if flag:
                    return 'a'
                return None


            def keep(s: str | None = None) -> str | None:
                t = s
                return t


            def nothing() -> str | None:
                return


            x: str | None = None
            x = pick(True)
            y = keep(x)
            z = keep()
            w: str | None = 'b'
            w = nothing()
            w = keep(pick(False))
            print('done')
        "},
    );
}

#[test]
fn a_value_of_any_union_goes_through_variables_parameters_and_results() {
    same_as_cpython(
        "a_value_of_any_union_goes_through_variables_parameters_and_results",
        indoc! {"
            def number(flag: bool) -> int | float:
                if flag:
                    return 1
                return 2.5


            def keep(x: int | float | bool | str | None = None) -> int | float | bool | str | None:
                y = x
                return y


            def maybe(s: str | None) -> str | int | None:
                return s


            n: int | None = None
            n = 3
            m: int | float = number(True)
            m = number(False)
            m = 7
            v = keep(m)
            v = keep(True)
            v = keep(False)
            v = keep('a')
            v = keep(-1073741824)
            v = keep(1073741823)
            v = keep()
            w = maybe('b')
            w = maybe(None)
            b: bool | int = True
            b = 0
            print('done')
        "},
    );
}

#[test]
fn is_none_tests_a_value_and_narrows_a_union() {
    same_as_cpython(
        "is_none_tests_a_value_and_narrows_a_union",
        indoc! {"
            def describe(s: str | None) -> str:
                if s is None:
                    return 'nothing'
                return s.upper()


            def next_of(n: int | None) -> int:
                if n is not None:
                    return n + 1
                return 0


            def half(x: float | None) -> float:
                if None is x:
                    return 0.0
                return x / 2


            def flip(b: bool | None) -> bool:
                if b is None:
                    return False
                return not b


            def first(s: str | int | None) -> bool:
                return s is None


            print(describe('abc'), describe(None))
            print(next_of(-1073741824), next_of(41), next_of(None))
            print(half(5.0), half(None))
            print(flip(True), flip(False), flip(None))
            print(first(None), first(3), first('a'))
            n = 1
            t = 'a'
            print(n is None, t is not None, None is None, None is not None)
            g: str | None = None
            if g is None:
                g = 'x'
            print(g.upper())
        "},
    );
}

#[test]
fn print_writes_a_value_of_a_union_as_its_type_says() {
    same_as_cpython(
        "print_writes_a_value_of_a_union_as_its_type_says",
        indoc! {"
            def show(x: int | float | bool | str | None) -> None:
                print(x)


            def maybe(s: str | None) -> None:
                print('<', s, '>')


            show(None)
            show(-1073741824)
            show(1073741823)
            show(0.1)
            show(-0.0)
            show(1e16)
            show(1e999 - 1e999)
            show(True)
            show(False)
            show('')
            show('é😀')
            maybe(None)
            maybe('a')
            n: int | None = None
            f: float | None = 2.5
            print(n, f, n, f)
            print(None, maybe('b'), None)
        "},
    );
}

#[test]
fn a_use_of_a_str_or_none_that_the_compiler_lacks_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            def f(s: str | None) -> str:
                return str(s)
        "}),
        "str of a union"
    );
    assert_eq!(
        unsupported(indoc! {"
            def f(s: str | None) -> str:
                return s or 'a'
        "}),
        "the truth of a union"
    );
    assert_eq!(
        unsupported(indoc! {"
            def f(self: str | None) -> None:
                pass
        "}),
        "a variable self of a union"
    );
    assert_eq!(
        unsupported(indoc! {"
            g: str | None = None


            def f() -> None:
                global g
                g = None


            if g is not None:
                f()
                print(g.upper())
        "}),
        "a read of a global of a union that a test narrows"
    );
    assert_eq!(
        unsupported(indoc! {"
            def f(self: int | None) -> None:
                pass
        "}),
        "a variable self of a union"
    );
}
