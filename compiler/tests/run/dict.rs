//! The values of `dict` and of `set`.

use indoc::indoc;

use super::{same_as_cpython, unsupported};

#[test]
fn a_function_with_a_dict_and_a_set_runs() {
    same_as_cpython(
        "a_function_with_a_dict_and_a_set_runs",
        indoc! {"
            def tamanho(d: dict[str, list[int]], s: set[str], t: set[int] | None) -> int:
                if t is None:
                    return len(d) + len(s)
                return len(d) + len(s) + len(t)

            def aninhado(d: dict[str, dict[int, float]]) -> int:
                return len(d)

            def limpa(d: dict[str, int], s: set[int] | None) -> None:
                d.clear()
                if s is not None:
                    s.clear()

            print('ok')
        "},
    );
}

#[test]
fn a_read_of_an_attribute_of_a_dict_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            def f(d: dict[str, int]) -> None:
                print(d.keys)
        "}),
        "this attribute of an object"
    );
}

#[test]
fn a_call_of_a_method_that_the_prelude_lacks_is_unsupported() {
    for (source, what) in [
        (
            indoc! {"
                def f(d: dict[str, int]) -> None:
                    d.setdefault('a', 1)
            "},
            "a call of this method",
        ),
        (
            indoc! {"
                def f(s: set[int]) -> None:
                    s.add(1)
            "},
            "a call of this method",
        ),
    ] {
        assert_eq!(unsupported(source), what, "{source}");
    }
}
