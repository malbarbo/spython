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
fn a_use_of_a_str_or_none_that_the_compiler_lacks_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            def f(s: str | None) -> None:
                print(s)
        "}),
        "print of a union"
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
            def f(x: int | float) -> None:
                print(x)
        "}),
        "print of a union"
    );
    assert_eq!(
        unsupported(indoc! {"
            def f(self: int | None) -> None:
                pass
        "}),
        "a variable self of a union"
    );
}
