//! Comprehensions, which run as a loop in line, and the consumers of a list
//! or of a generator expression.

use indoc::indoc;

use std::path::Path;

use super::{compile, same_as_cpython, stops_at_the_int_limit, unsupported};

#[test]
fn a_list_comprehension_makes_a_list() {
    same_as_cpython(
        "a_list_comprehension_makes_a_list",
        indoc! {"
            xs = [1, 2, 3]
            x = 'fora'
            ys = [x * 2 for x in xs if x > 1]
            print(ys, x)
            print([[i * j for j in range(i)] for i in range(4)])
            print([c for c in 'abc'], [a + b for a in 'xy' for b in 'zw' if a != 'y'])
            print([c for c in 'abcdef' if c != 'b' if c != 'e'])
            d = {'a': 1, 'b': 2}
            print([k for k in d], [v for v in d.values()], [k + str(v) for k, v in d.items()])
            print([s for s in {3}])

            def metade(n: int) -> list[float]:
                k = 0.5
                return [i * k for i in range(n)]

            print(metade(3), [x for x in [x for x in xs]], [n for n in range(3) if n < 0])
            zs: list[int | None] = [1, None, 3]
            print([z + 1 for z in zs if z is not None])
            print([True for _ in range(2)], [b + 1 for b in [True, False]])
        "},
    );
}

#[test]
fn the_first_iterable_of_a_comprehension_reads_the_names_around_it() {
    same_as_cpython(
        "the_first_iterable_of_a_comprehension_reads_the_names_around_it",
        indoc! {"
            x = [1, 2]
            print([x * 10 for x in x])
            print([y for x in [[3, 4], [5]] for y in x], x)
        "},
    );
}

#[test]
fn a_comprehension_calls_its_variable_that_holds_a_function() {
    same_as_cpython(
        "a_comprehension_calls_its_variable_that_holds_a_function",
        indoc! {"
            from typing import Callable

            def f(x: int) -> int:
                return x + 100

            def dobro(x: int) -> int:
                return 2 * x

            fs = [dobro]
            print([f(3) for f in fs])

            def aplica(gs: list[Callable[[int], int]]) -> list[int]:
                return [h(3) for h in gs]

            print(aplica([dobro, f]))
        "},
    );
}

#[test]
fn a_comprehension_inside_a_lambda_captures_the_names_it_reads() {
    same_as_cpython(
        "a_comprehension_inside_a_lambda_captures_the_names_it_reads",
        indoc! {"
            from typing import Callable

            def soma_a(k: int) -> Callable[[list[int]], list[int]]:
                return lambda xs: [x + k for x in xs]

            def pares(n: int) -> list[int]:
                def eh_par(i: int) -> bool:
                    return i % 2 == 0
                return [i for i in range(n) if eh_par(i)]

            print(soma_a(10)([1, 2]), pares(7))
        "},
    );
}

#[test]
fn an_error_in_a_comprehension_has_no_frame_of_its_own() {
    same_as_cpython(
        "an_error_in_a_comprehension_has_no_frame_of_its_own",
        indoc! {"
            def inversos(n: int) -> list[list[int]]:
                return [[1 // (i - j) for j in range(2)] for i in range(n)]

            print(inversos(1))
            print(inversos(3))
        "},
    );
}

#[test]
fn set_and_dict_comprehensions_make_a_set_and_a_dict() {
    same_as_cpython(
        "set_and_dict_comprehensions_make_a_set_and_a_dict",
        indoc! {"
            xs = [3, 1, 3, -1, -2, 1]
            print({x * x for x in xs}, {x for x in xs if x > 5})
            print({c: ord(c) for c in 'abca'})
            d = {k: v * 2 for k, v in {'a': 1, 'b': 2}.items() if k != 'a'}
            print(d, {n: [m for m in range(n)] for n in range(3)})
            print({s.upper() for s in ['oi', 'tchau', 'oi']} == {'OI', 'TCHAU'})
            print(len({1.0 for _ in range(3)}), {True: 1 for _ in range(1)})
            print({b for b in [True, False, True]}, {n: n for n in {5, 6}})
        "},
    );
}

#[test]
fn a_comprehension_hashes_each_key_as_it_comes() {
    let source = |head: &str, tail: &str| {
        format!(
            indoc! {"
                class K:
                    n: int

                    def __init__(self, n: int) -> None:
                        self.n = n

                    def __eq__(self, other: object) -> bool:
                        return isinstance(other, K) and self.n == other.n

                    def __hash__(self) -> int:
                        print('hash', self.n)
                        if self.n == 2:
                            raise ValueError('ruim')
                        return self.n

                def v(n: int) -> int:
                    print('v', n)
                    return n

                c = {}
                     {}
            "},
            head, tail
        )
    };
    same_as_cpython(
        "a_set_comprehension_hashes_each_key_as_it_comes",
        &source("{K(v(n))", "for n in range(3)}"),
    );
    same_as_cpython(
        "a_dict_comprehension_hashes_each_key_as_it_comes",
        &source("{K(v(n)): v(n * 10)", "for n in range(3)}"),
    );
}

#[test]
fn a_lambda_that_reads_a_name_of_a_comprehension_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            fs = [lambda: i for i in range(3)]
        "}),
        "a lambda that reads a name of a comprehension"
    );
}

#[test]
fn sum_min_max_any_and_all_consume_a_list() {
    same_as_cpython(
        "sum_min_max_any_and_all_consume_a_list",
        indoc! {"
            import math

            xs = [3, 1, 4, 1, 5]
            fs = [0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1]
            vazia: list[float] = []
            print(sum(xs), sum([True, True, False]), sum(fs), sum(vazia), sum([-0.0]))
            print(sum([1e308, 1e308, -1e308]), sum([1e100, 1.0, -1e100, 1.0]))
            print(sum([math.inf, 1.0]), sum([math.nan, 1.0]), sum([2.5]), sum([-1e-20, -0.0]))
            print(min(xs), max(xs), min(['b', 'a', 'c']), max([True, False]))
            print(max([1.0, math.nan]), max([math.nan, 1.0]), min([-0.0, 0.0]), max([0.0, -0.0]))
            print(any(xs), all(xs), any([0, 0]), all(vazia), any(['', 'a']), all([xs, [1]]))
            print(any(['', '']), all([0.0]))
            y = sum(vazia)
            print(y)
            print(max(vazia))
        "},
    );
}

#[test]
fn min_and_max_of_objects_compare_with_their_methods() {
    same_as_cpython(
        "min_and_max_of_objects_compare_with_their_methods",
        indoc! {"
            class P:
                n: int

                def __init__(self, n: int) -> None:
                    self.n = n

                def __lt__(self, other: 'P') -> bool:
                    print('lt', self.n, other.n)
                    return 1 // (self.n - 3) < other.n

                def __repr__(self) -> str:
                    return f'P({self.n})'

            class B:
                def __bool__(self) -> bool:
                    print('bool')
                    return False

            print(min([P(2), P(1), P(4)]), max([P(2), P(1), P(4)]))
            print(any([B(), B()]), all([B(), B()]))
            print(max([P(3), P(2)]))
        "},
    );
}

#[test]
fn sum_of_ints_stops_at_the_int_limit() {
    stops_at_the_int_limit(
        "sum_of_ints_stops_at_the_int_limit",
        indoc! {"
            xs = [1073741823, 1]
            print(sum(xs))
        "},
        "",
        2,
        "    print(sum(xs))\n          ~~~^^^^\n",
        "1073741824",
    );
}

#[test]
fn a_consumer_of_another_iterable_is_unsupported() {
    assert_eq!(
        unsupported("print(min('abc'))\n"),
        "min or max of this iterable"
    );
    assert_eq!(unsupported("print(sum([1, 2], 10))\n"), "this call of sum");
}

#[test]
fn a_consumer_takes_a_generator_expression() {
    same_as_cpython(
        "a_consumer_takes_a_generator_expression",
        indoc! {"
            xs = [3, 1, 4, 1, 5]
            d = {'a': 1, 'b': 2}
            print(sum(x * x for x in xs), sum(x / 2 for x in xs))
            print(min(x for x in xs if x > 1), max(-x for x in xs))
            print(sum(i for i in range(1, 11)), sum(i for i in range(10, 0, -2)))
            print(sum(v for v in d.values()), sum(len(k) * v for k, v in d.items()))
            print(any(x > 4 for x in xs), all(x > 0 for x in xs), any(k == 'b' for k in d))
            print(all(c != 'z' for c in 'abc'), any(x > 9 for x in xs), all(s for s in {1, 2}))
            print(sum(x * y for x in xs for y in range(x) if y % 2 == 0))
            print(', '.join(str(x) for x in xs), '-'.join(c.upper() for c in 'abc' if c != 'b'))

            def f(n: int, k: int) -> int:
                return sum(i * k for i in range(n))

            print(f(4, 3), [sum(x * y for y in xs) for x in range(3)])
            print([max(c * v for v in range(3)) for c in range(4)])
        "},
    );
}

#[test]
fn any_and_all_stop_at_the_value_that_decides() {
    same_as_cpython(
        "any_and_all_stop_at_the_value_that_decides",
        indoc! {"
            def maior(x: int) -> bool:
                print('testa', x)
                return 1 // x > 0

            xs = [2, 1, 0]
            print(any(maior(x) for x in xs), all(not maior(x) for x in xs))
            print(all(maior(x) for x in xs))
        "},
    );
}

#[test]
fn an_error_in_a_generator_expression_has_the_frame_genexpr() {
    same_as_cpython(
        "an_error_in_a_generator_expression_has_the_frame_genexpr",
        indoc! {"
            def inversos(xs: list[int]) -> int:
                return sum(1 // x for x in xs)

            print(inversos([1, 2]))
            print(inversos([1, 0]))
        "},
    );
    same_as_cpython(
        "the_first_iterable_of_a_generator_runs_in_the_frame_around_it",
        indoc! {"
            def itens(n: int) -> list[int]:
                return [1 // n]

            print(max(x for x in itens(0)))
        "},
    );
    same_as_cpython(
        "the_parts_of_a_range_of_a_generator_run_in_order",
        indoc! {"
            def f(n: int) -> int:
                print('f', n)
                return n

            print(sum(x for x in range(f(1), f(4))))
        "},
    );
    same_as_cpython(
        "max_of_an_empty_generator_raises_in_the_frame_around_it",
        indoc! {"
            xs = [1, 2]
            print(max(x for x in xs if x > 5))
        "},
    );
}

#[test]
fn a_name_error_in_a_generator_expression_suggests_its_locals() {
    same_as_cpython(
        "a_name_error_in_a_generator_expression_suggests_its_locals",
        indoc! {"
            def f(xs: list[int]) -> int:
                return sum(valorr for valor in xs)

            print(f([1]))
            valorr = 2
        "},
    );
    same_as_cpython(
        "a_name_error_in_a_generator_expression_suggests_its_parameter",
        indoc! {"
            def f(xs: list[int]) -> int:
                return sum(x0 for x in xs)

            print(f([1]))
            x0 = 2
        "},
    );
}

#[test]
fn a_generator_expression_counts_as_a_frame_for_the_recursion_limit() {
    same_as_cpython(
        "a_generator_expression_counts_as_a_frame_for_the_recursion_limit",
        indoc! {"
            def r(n: int) -> int:
                return 0 if n == 0 else sum(r(n - 1) + 1 for _ in range(1))

            print(r(3))
            print(r(2000))
        "},
    );
}

#[test]
fn a_generator_expression_outside_a_consumer_is_unsupported() {
    assert_eq!(
        unsupported("xs = [1]\ng = (x for x in xs)\n"),
        "this expression"
    );
    assert_eq!(
        unsupported(indoc! {"
            class P:
                def __lt__(self, other: 'P') -> bool:
                    return False

            print(min(p for p in [P()]))
        "}),
        "min or max of a generator of objects"
    );
    assert_eq!(
        unsupported(indoc! {"
            class P:
                pass

            print(any(p for p in [P()]))
        "}),
        "any or all of a generator of objects"
    );
    assert_eq!(
        unsupported("k = 2\nprint(sum(i for i in range(0, 10, k)))\n"),
        "a generator expression over a range with this step"
    );
}

#[test]
fn sorted_sorts_a_list_and_a_generator_expression() {
    same_as_cpython(
        "sorted_sorts_a_list_and_a_generator_expression",
        indoc! {"
            import math

            xs = [5, 2, 9, 1, 5, 6]
            print(sorted(xs), xs)
            ys = [3.0, math.nan, 1.0, 2.0]
            print(sorted(ys), sorted(ys, reverse=True))
            ws = ['banana', 'Abacaxi', 'uva', 'kiwi']
            print(sorted(ws), sorted(ws, key=lambda w: len(w)))
            print(sorted(ws, key=lambda w: w.lower(), reverse=True))
            print(sorted(ws, key=lambda w: len(w), reverse=True), sorted(ws, key=None))
            print(sorted(x * x for x in [-3, 1, -2]), sorted((x for x in xs), reverse=True))
            vazia: list[int] = []
            print(sorted(vazia), sorted([7]), sorted([True, False, True]))
        "},
    );
}

#[test]
fn sort_sorts_the_list_in_place() {
    same_as_cpython(
        "sort_sorts_the_list_in_place",
        indoc! {"
            import math


            def ordena(xs: list[int]) -> None:
                xs.sort(reverse=True)


            xs = [5, 2, 9, 1, 5, 6]
            ys = xs
            xs.sort()
            print(xs, ys)
            ordena(ys)
            print(xs)
            fs = [3.0, math.nan, 1.0, 2.0]
            fs.sort()
            print(fs)
            ws = ['banana', 'Abacaxi', 'uva', 'kiwi']
            ws.sort(key=lambda w: len(w))
            print(ws)
            ws.sort(key=lambda w: w.lower(), reverse=True)
            print(ws)
            ws.sort(key=None)
            print(ws)
            ns = [(i * 37) % 101 for i in range(200)]
            ns.sort(key=lambda n: n % 7)
            print(ns)
            vazia: list[int] = []
            vazia.sort()
            print(vazia)
        "},
    );
}

#[test]
fn sorted_makes_the_comparisons_of_cpython() {
    same_as_cpython(
        "sorted_makes_the_comparisons_of_cpython",
        indoc! {"
            class Contador:
                n: int
                h: int

                def __init__(self) -> None:
                    self.n = 0
                    self.h = 0


            contador = Contador()


            class C:
                v: int
                i: int

                def __init__(self, v: int, i: int) -> None:
                    self.v = v
                    self.i = i

                def __lt__(self, other: 'C') -> bool:
                    contador.n += 1
                    contador.h = (contador.h * 31 + self.i * 7 + other.i) % 1000003
                    if self.v == -1 and other.v == -1:
                        return (self.i * 13 + other.i * 7 + contador.n) % 3 == 0
                    return self.v < other.v


            def gera(n: int, semente: int, modo: int) -> list[C]:
                xs: list[C] = []
                s = semente
                for i in range(n):
                    s = (s * 75 + 74) % 65537
                    if modo == 0:
                        v = s % 10
                    elif modo == 1:
                        v = s
                    elif modo == 2:
                        v = i if s % 10 != 0 else s % 1000
                    elif modo == 3:
                        v = n - i if s % 7 != 0 else s % 50
                    elif modo == 4:
                        v = i % 37
                    elif modo == 5:
                        v = (i // 50) * 1000 - i if i % 100 < 50 else i
                    elif modo == 6:
                        v = 0 if i < n // 2 else 1000 - i
                    elif modo == 7:
                        v = 2 * i if i < n // 2 else 2 * (i - n // 2) + 1
                    elif modo == 8:
                        v = i if i < n // 3 or i > n // 3 + 40 else i - n // 4
                    else:
                        v = -1
                    xs.append(C(v, i))
                return xs


            def marca(xs: list[C]) -> int:
                h = 0
                for p in range(len(xs)):
                    h = (h * 131 + xs[p].i) % 1000003
                return h


            for n in [0, 1, 2, 5, 31, 64, 65, 129, 700, 1500]:
                for modo in range(10):
                    xs = gera(n, n + modo, modo)
                    contador.n = 0
                    contador.h = 0
                    ys = sorted(xs)
                    a1 = contador.n
                    a2 = contador.h
                    contador.n = 0
                    contador.h = 0
                    zs = sorted(xs, reverse=True)
                    b1 = contador.n
                    b2 = contador.h
                    ws = sorted(xs, key=lambda c: c.v % 13)
                    us = sorted(xs, key=lambda c: -c.v, reverse=True)
                    print(n, modo, a1, a2, marca(ys), b1, b2, marca(zs), marca(ws), marca(us))
        "},
    );
}

#[test]
fn an_error_in_a_key_or_in_lt_raises_through_sorted() {
    same_as_cpython(
        "an_error_in_a_key_raises_through_sorted",
        indoc! {"
            def chave(x: int) -> int:
                print('chave', x)
                return 10 // x

            print(sorted([3, 1, 0, 2], key=lambda x: chave(x)))
        "},
    );
    same_as_cpython(
        "an_error_in_lt_raises_through_sorted",
        indoc! {"
            class P:
                n: int

                def __init__(self, n: int) -> None:
                    self.n = n

                def __lt__(self, other: 'P') -> bool:
                    return 1 // (self.n - other.n) > 5

            print(sorted([P(1), P(2), P(2)]))
        "},
    );
}

#[test]
fn the_order_of_sorted_of_a_generator_is_the_order_of_cpython() {
    same_as_cpython(
        "the_order_of_sorted_of_a_generator_is_the_order_of_cpython",
        indoc! {"
            from typing import Callable

            def f(n: int) -> list[int]:
                print('iterável')
                return [n, 1]

            def dobro(n: int) -> int:
                print('valor', n)
                return 2 * n

            def chave() -> Callable[[int], int]:
                print('chave')
                return lambda x: -x

            def reverso() -> bool:
                print('reverso')
                return True

            print(sorted((dobro(x) for x in f(3)), key=chave(), reverse=reverso()))
        "},
    );
}

#[test]
fn sorted_of_another_iterable_is_unsupported() {
    assert_eq!(
        unsupported(
            "print(sorted('cba'))
"
        ),
        "sorted of this iterable"
    );
    assert_eq!(
        unsupported(
            "print(sorted([[2], [1]], key=lambda x: x))
"
        ),
        "a key of this type"
    );
}

#[test]
fn sorted_of_items_that_the_prelude_cannot_order_is_unsupported() {
    for call in ["sorted([[2], [1]])", "sorted(x for x in [[1]])"] {
        let source = format!("print({call})\n");
        let Err(compiler::Error::Unsupported { range, what }) =
            compile(&source, Path::new("sorted.py"))
        else {
            panic!("sorted of a list of lists compiled");
        };
        assert_eq!(what, "sorted of a list of this type");
        assert_eq!(&source[range], call);
    }
}

#[test]
fn min_and_max_with_a_key_compare_the_keys() {
    same_as_cpython(
        "min_and_max_with_a_key_compare_the_keys",
        indoc! {"
            import math
            from typing import Callable


            def tam(w: str) -> int:
                return len(w)


            def absoluto(n: int) -> int:
                return abs(n)


            def mesmo(x: float) -> float:
                return x


            def k(w: str) -> int:
                print('k', w)
                return len(w)


            class P:
                n: int

                def __init__(self, n: int) -> None:
                    self.n = n

                def __repr__(self) -> str:
                    return f'P({self.n})'


            def valor(p: P) -> int:
                return p.n


            ws = ['banana', 'uva', 'kiwi', 'abacate', 'figo']
            print(min(ws, key=tam), max(ws, key=tam), max(ws, key=None))
            print(min('b', 'aa', 'c', key=tam), max(3, -7, 5, key=absoluto))
            fs = [2.0, math.nan, 1.0]
            print(max(fs, key=mesmo), min(fs, key=mesmo))
            print(max(ws, key=k))
            ps = [P(3), P(1), P(2)]
            print(min(ps, key=valor), max(ps, key=valor))
            f: Callable[[str], int] = lambda w: -len(w)
            print(min(ws, key=f))
            vazia: list[str] = []
            print(max(vazia, key=tam))
        "},
    );
}

#[test]
fn min_and_max_of_a_generator_with_a_key_are_unsupported() {
    let source = |call: &str| {
        format!(
            indoc! {"
                def tam(w: str) -> int:
                    return len(w)

                ws = ['a']
                print({})
            "},
            call
        )
    };
    assert_eq!(
        unsupported(&source("min((w for w in ws), key=tam)")),
        "min or max of a generator with a key"
    );
    assert_eq!(
        unsupported(&source("max(ws, key=tam, default='')")),
        "min or max with these keyword arguments"
    );
}
