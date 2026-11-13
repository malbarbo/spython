//! Functions as values: the name of a function, the call of a value, a
//! `lambda` and a `def` inside a function, with the variables that they
//! capture.

use indoc::indoc;

use super::{same_as_cpython, unsupported};

#[test]
fn the_name_of_a_function_is_a_value() {
    same_as_cpython(
        "the_name_of_a_function_is_a_value",
        indoc! {"
            from typing import Callable

            def dobro(n: int) -> int:
                return 2 * n

            def triplo(n: int) -> int:
                return 3 * n

            fs: list[Callable[[int], int]] = [dobro, triplo, dobro]
            g = dobro
            h: Callable[[int], int] = triplo
            fs.append(h)
            print(len(fs), bool(g), h is None)
            print(g == dobro, g != triplo, g is h, fs == [dobro, triplo, dobro, h])
            tabela: dict[str, Callable[[int], int]] = {'dobro': dobro}
            print(tabela == {'dobro': g}, tabela == {'dobro': h})
        "},
    );
}

#[test]
fn a_read_of_a_function_before_its_def_raises_name_error() {
    same_as_cpython(
        "a_read_of_a_function_before_its_def_raises_name_error",
        indoc! {"
            def usa() -> None:
                g = depois
                print('nunca')

            usa()

            def depois() -> None:
                print('depois')
        "},
    );
}

#[test]
fn print_of_a_function_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            def f() -> None:
                pass

            print(f)
        "}),
        "print of a function"
    );
}
