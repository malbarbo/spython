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

#[test]
fn a_call_of_a_function_value_runs_its_function() {
    same_as_cpython(
        "a_call_of_a_function_value_runs_its_function",
        indoc! {"
            from typing import Callable

            def dobro(n: int) -> int:
                return 2 * n

            def soma(a: int, b: int) -> int:
                return a + b

            def diga(s: str) -> None:
                print('diga', s)

            def aplica(f: Callable[[int], int], v: int) -> int:
                return f(v)

            def escolhe(b: bool) -> Callable[[int, int], int]:
                return soma

            def divide(n: int) -> int:
                return 10 // n

            fs: list[Callable[[int], int]] = [dobro, divide]
            g = dobro
            print(g(3), aplica(dobro, 4), fs[0](5), escolhe(True)(1, 2))
            h: Callable[[str], None] = diga
            h('oi')
            print(aplica(divide, 0))
        "},
    );
}

#[test]
fn a_call_of_a_function_value_counts_against_the_recursion_limit() {
    same_as_cpython(
        "a_call_of_a_function_value_counts_against_the_recursion_limit",
        indoc! {"
            from typing import Callable

            def conta(n: int) -> int:
                if n == 0:
                    return 0
                return 1 + f(n - 1)

            f: Callable[[int], int] = conta
            print(f(500))
            print(f(2000))
        "},
    );
}

#[test]
fn a_call_of_a_variable_before_its_assignment_raises_name_error() {
    same_as_cpython(
        "a_call_of_a_variable_before_its_assignment_raises_name_error",
        indoc! {"
            def conta(n: int) -> int:
                return n

            def usa() -> int:
                return g(1)

            print(usa())
            g = conta
        "},
    );
}

#[test]
fn a_call_of_a_local_before_its_assignment_raises_unbound_local_error() {
    same_as_cpython(
        "a_call_of_a_local_before_its_assignment_raises_unbound_local_error",
        indoc! {"
            def g() -> None:
                print('g')

            def f() -> None:
                while True:
                    h()  # ty: ignore[possibly-unresolved-reference]
                    h = g

            f()
        "},
    );
}

#[test]
fn a_call_of_a_function_value_with_a_keyword_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            def f(n: int) -> int:
                return n

            g = f
            print(g(n=1))
        "}),
        "a call of a function value with these arguments"
    );
}
