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
fn a_dict_literal_with_a_starred_item_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            a = {'a': 1}
            b = {**a, 'b': 2}
        "}),
        "a starred item"
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
                chave: int | str
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
                h: set[int | str | None],
                i: dict[Cor | No | float, int],
            ) -> int:
                return len(a) + len(b) + len(c) + len(d) + len(e) + len(f) + len(g) + len(h) + len(i)

            print('ok')
        "},
    );
}

/// CPython raises `TypeError` when it takes the `hash` of an object
/// without one, and the compiler refuses the type of a `set` or of a key
/// of a `dict` of an object without a `hash`. A value of `object` may be
/// a list.
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
        indoc! {"
            from dataclasses import dataclass

            @dataclass
            class Ponto:
                x: int

            def f(s: set[int | Ponto]) -> None:
                pass
        "},
        indoc! {"
            def f(s: set[object]) -> None:
                pass
        "},
        indoc! {"
            def f(s: set[int | list[int]]) -> None:
                pass
        "},
    ] {
        assert_eq!(unsupported(source), "a parameter of this type", "{source}");
    }
}

#[test]
fn a_dict_literal_with_len_and_subscripts_runs() {
    same_as_cpython(
        "a_dict_literal_with_len_and_subscripts_runs",
        indoc! {"
            def chave(s: str) -> str:
                print('chave', s)
                return s

            def valor(n: int) -> int:
                print('valor', n)
                return n

            d: dict[str, int] = {}
            print(len(d))
            d['a'] = 1
            d['b'] = 2
            d['a'] = 3
            print(len(d), d['a'], d['b'])
            e = {chave('x'): valor(1), chave('y'): valor(2), chave('x'): valor(3)}
            print(len(e), e['x'], e['y'])
            f: dict[int, str] = {}
            for i in range(100):
                f[i * 7] = str(i)
            print(len(f), f[0], f[693], f[350])
            g = {1: 'a', 1.0: 'b', True: 'c'}
            print(len(g), g[1])
            h = {1: 2, 3: 4, 5: 6, 7: 8, 9: 10, 11: 12, 13: 14}
            print(len(h), h[13])
            print(d['z'])
        "},
    );
}

/// The `hash` and `==` of a key run as in CPython: a literal takes the
/// `hash` of each key after it evaluates every item, and a search puts the
/// key of the `dict` on the left of `==`.
#[test]
fn a_dict_calls_hash_and_eq_of_a_key_as_cpython_does() {
    same_as_cpython(
        "a_dict_calls_hash_and_eq_of_a_key_as_cpython_does",
        indoc! {"
            from enum import Enum

            class Cor(Enum):
                AZUL = 1
                VERDE = 2

            class Ponto:
                x: int

                def __init__(self, x: int) -> None:
                    print('ponto', x)
                    self.x = x

                def __eq__(self, other: object) -> bool:
                    print('eq', self.x, isinstance(other, Ponto) and other.x)
                    return isinstance(other, Ponto) and self.x == other.x

                def __hash__(self) -> int:
                    print('hash', self.x)
                    return self.x % 2

                def __repr__(self) -> str:
                    return 'Ponto(' + str(self.x) + ')'

            d = {Ponto(1): 'a', Ponto(3): 'b', Ponto(1): 'c'}
            print(len(d))
            print(d[Ponto(3)])
            d[Ponto(5)] = 'd'
            print(len(d))
            cores = {Cor.AZUL: 1.5, Cor.VERDE: -0.0}
            print(cores[Cor.VERDE], cores[Cor.AZUL])
            n: dict[str | None, int] = {None: 1, 'a': 2}
            print(n[None], n['a'])
            u: dict[int | str, float] = {}
            for i in range(20):
                u[i] = i / 2
                u[str(i)] = -i / 2
            print(len(u), u[7], u['7'])
            print(d[Ponto(7)])
        "},
    );
}

#[test]
fn a_dict_writes_its_items_in_the_order_of_insertion() {
    same_as_cpython(
        "a_dict_writes_its_items_in_the_order_of_insertion",
        indoc! {"
            from dataclasses import dataclass

            @dataclass(frozen=True)
            class Ponto:
                x: int
                y: int

            vazio: dict[str, int] = {}
            print(vazio, repr(vazio), str(vazio), f'{vazio}')
            d = {'b': 1, 'a': 2, \"it's\": 3}
            d['b'] = 4
            print(d)
            print(repr(d), str(d), f'<{d}>')
            print([d, {'x': [1.5, -0.0]}])
            aninhado: dict[int, dict[float, str | None]] = {1: {2.5: None}, 3: {}}
            print(aninhado)
            pontos = {Ponto(1, 2): 'p', Ponto(-1, 0): 'q'}
            print(pontos)
            u: dict[int | str | None, list[int] | None] = {None: None, 1: [1], 'um': []}
            print(u)
            f: dict[int, int] = {}
            for i in range(30):
                f[i * i % 17] = i
            print(f)
            o: dict[str, object] = {}
            o['eu'] = o
            o['x'] = d
            print(o)
        "},
    );
}
