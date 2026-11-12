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

/// An item of a `set` and a key of a `dict` may be of any type with a
/// `hash`, and an object of a class may be in one when each class that
/// descends from its class has a `hash`.
#[test]
fn a_set_and_a_dict_of_a_type_with_a_hash_run() {
    same_as_cpython(
        "a_set_and_a_dict_of_a_type_with_a_hash_run",
        indoc! {"
            from dataclasses import dataclass
            from enum import Enum, auto

            class Cor(Enum):
                AZUL = auto()
                VERDE = auto()

            @dataclass(frozen=True)
            class No:
                valor: int
                nome: str | None
                cor: Cor
                proximo: 'No | None'

            class Ponto:
                x: int

                def __init__(self, x: int) -> None:
                    self.x = x

                def __eq__(self, other: object) -> bool:
                    return isinstance(other, Ponto) and self.x == other.x

                def __hash__(self) -> int:
                    return self.x

            class Ponto3(Ponto):
                pass

            def tamanho(
                a: set[int],
                b: set[float],
                c: set[bool],
                d: set[str | None],
                e: set[Cor],
                f: set[No],
                g: dict[Ponto, list[int]],
            ) -> int:
                return len(a) + len(b) + len(c) + len(d) + len(e) + len(f) + len(g)

            print('ok')
        "},
    );
}

/// CPython raises `TypeError` when it takes the `hash` of an object
/// without one, and the compiler refuses the type of a `set` or of a key
/// of a `dict` of an object without a `hash`.
#[test]
fn a_set_and_a_dict_key_without_a_hash_are_unsupported() {
    for source in [
        indoc! {"
            def f(s: set[list[int]]) -> None:
                pass
        "},
        indoc! {"
            def f(d: dict[list[int], int]) -> None:
                pass
        "},
        indoc! {"
            class Ponto:
                x: int

                def __init__(self, x: int) -> None:
                    self.x = x

                def __eq__(self, other: object) -> bool:
                    return isinstance(other, Ponto) and self.x == other.x

            def f(s: set[Ponto]) -> None:
                pass
        "},
        indoc! {"
            class Ponto:
                x: int

                def __init__(self, x: int) -> None:
                    self.x = x

            def f(s: set[Ponto]) -> None:
                pass
        "},
        indoc! {"
            class Ponto:
                x: int

                def __init__(self, x: int) -> None:
                    self.x = x

                def __hash__(self) -> int:
                    return self.x

            class Ponto3(Ponto):
                def __eq__(self, other: object) -> bool:
                    return False

            def f(s: set[Ponto]) -> None:
                pass
        "},
        indoc! {"
            from dataclasses import dataclass

            @dataclass
            class Ponto:
                x: int

            def f(d: dict[Ponto, int]) -> None:
                pass
        "},
        indoc! {"
            from dataclasses import dataclass

            @dataclass(frozen=True)
            class Ponto:
                xs: list[int]

            def f(s: set[Ponto]) -> None:
                pass
        "},
    ] {
        assert_eq!(unsupported(source), "a parameter of this type", "{source}");
    }
}
