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

#[test]
fn in_get_pop_clear_and_copy_of_a_dict_run() {
    same_as_cpython(
        "in_get_pop_clear_and_copy_of_a_dict_run",
        indoc! {"
            def padrao() -> int:
                print('padrao')
                return -1

            d = {'a': 1, 'b': 2, 'c': 3}
            print('a' in d, 'z' in d, 'a' not in d)
            print(d.get('a'), d.get('z'), d.get('z', None), d.get('b', 0), d.get('z', padrao()))
            x = d.get('z', 'nada')
            print(x)
            print(d.pop('b'), d, len(d))
            print(d.pop('b', 7), d.pop('b', None), d.pop('c', None), d)
            e = d.copy()
            e['n'] = 9
            print(d, e)
            muitos: dict[int, str] = {}
            for i in range(40):
                muitos[i] = str(i)
            for i in range(0, 40, 3):
                muitos.pop(i)
            print(muitos.copy())
            for i in range(0, 40, 2):
                muitos.pop(i, '')
            copia = muitos.copy()
            copia[100] = 'cem'
            print(copia, len(copia))
            d.clear()
            print(d, len(d), d.get('a'))
            d['q'] = 5
            print(d)
            vazio: dict[str, int] = {}
            print(vazio.copy(), vazio.pop('a', 0))
            f = {1.5: 'x'}
            print(f.pop(1.5))
            f.pop(2.5)
        "},
    );
}

#[test]
fn a_pop_of_a_missing_key_raises_key_error_with_its_repr() {
    same_as_cpython(
        "a_pop_of_a_missing_key_raises_key_error_with_its_repr",
        indoc! {"
            from dataclasses import dataclass

            @dataclass(frozen=True)
            class Ponto:
                x: int

            d = {Ponto(1): \"it's\"}
            print(d[Ponto(1)])
            print(d.pop(Ponto(2)))
        "},
    );
}

#[test]
fn a_for_over_a_dict_its_keys_and_its_values_runs() {
    same_as_cpython(
        "a_for_over_a_dict_its_keys_and_its_values_runs",
        indoc! {"
            def soma(d: dict[str, int]) -> int:
                total = 0
                for v in d.values():
                    total += v
                return total

            d: dict[str, int] = {'b': 1, 'a': 2, 'c': 3}
            for k in d:
                print(k, d[k])
            for k in d.keys():
                if k == 'a':
                    continue
                print('chave', k)
            else:
                print('fim')
            for v in d.values():
                if v == 2:
                    break
                print('valor', v)
            else:
                print('nunca')
            print(soma(d))
            d.pop('b')
            d['b'] = 10
            for k in d:
                d[k] = d[k] * 2
            print(d)
            vazio: dict[int, float] = {}
            for x in vazio:
                print(x)
            else:
                print('vazio')
            grande: dict[int, list[int]] = {}
            for i in range(50):
                grande[i] = [i]
            for i in range(0, 50, 2):
                grande.pop(i)
            n = 0
            for i in grande:
                n += i + grande[i][0]
            print(n)
        "},
    );
}

/// A `for` over a `dict` that changes raises `RuntimeError` at the
/// iterable, as in CPython, even when the change comes in the last turn.
#[test]
fn a_for_over_a_dict_that_changes_raises_runtime_error() {
    for (name, body) in [
        ("grows", "d[k + k] = 0"),
        ("shrinks_last", "if k == 'c':\n        d.pop('a')"),
        ("grows_last", "if k == 'c':\n        d['z'] = 0"),
        ("same_size", "d.pop(k)\n    d[k + 'x'] = 0"),
        ("values", "d.clear()"),
    ] {
        let iter = if name == "values" { "d.values()" } else { "d" };
        same_as_cpython(
            &format!("a_for_over_a_dict_that_changes_raises_runtime_error_{name}"),
            &format!(
                "d: dict[str, int] = {{'a': 1, 'b': 2, 'c': 3}}\nfor k in {iter}:\n    print(k)\n    {body}\nprint('fim')\n"
            ),
        );
    }
}

#[test]
fn a_for_over_the_items_of_a_dict_runs() {
    same_as_cpython(
        "a_for_over_the_items_of_a_dict_runs",
        indoc! {"
            def total(d: dict[str, float]) -> float:
                soma = 0.0
                for nome, valor in d.items():
                    soma += valor
                    print(nome, valor)
                return soma

            precos: dict[str, float] = {'pao': 0.5, 'leite': 4.25}
            print(total(precos))
            idades: dict[str, int | None] = {'ana': 3, 'bia': None}
            for nome, idade in idades.items():
                if idade is None:
                    print(nome, 'sem idade')
                    continue
                print(nome, idade + 1)
            else:
                print('fim')
            for nome, idade in idades.items():
                idades.pop(nome)
        "},
    );
}

#[test]
fn a_for_over_the_items_of_a_dict_with_one_name_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            d = {'a': 1}
            for par in d.items():
                print(par)
        "}),
        "this target"
    );
}

#[test]
fn equal_and_not_equal_of_dicts_run() {
    same_as_cpython(
        "equal_and_not_equal_of_dicts_run",
        indoc! {"
            class Valor:
                n: int

                def __init__(self, n: int) -> None:
                    self.n = n

                def __eq__(self, other: object) -> bool:
                    print('eq', self.n)
                    return isinstance(other, Valor) and self.n == other.n

            a = {'x': 1, 'y': 2}
            b = {'y': 2, 'x': 1}
            c = {'x': 1, 'y': 3}
            print(a == b, a != b, a == c, a != c, a == a)
            b.pop('x')
            print(a == b)
            b['x'] = 1
            print(a == b, b)
            vazio: dict[str, int] = {}
            print(vazio == vazio.copy(), a == vazio)
            print([a, c] == [b, c], [a] == [c])
            o: object = a
            print(o == b, o == c, o == 1)
            u: int | dict[str, int] = a
            print(u == b, u == 3, u != c)
            v = {1: Valor(1), 2: Valor(2)}
            w = {2: Valor(2), 1: Valor(1)}
            print(v == w)
            f = {'n': float('nan')}
            print(f == f)
        "},
    );
}

#[test]
fn a_comparison_of_dicts_of_two_types_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            a = {'x': 1}
            b = {'x': 1.0}
            print(a == b)
        "}),
        "a comparison of dicts of two types"
    );
}
