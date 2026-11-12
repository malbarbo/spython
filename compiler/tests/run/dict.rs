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
