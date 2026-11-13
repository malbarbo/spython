//! Comprehensions, which run as a loop in line, and the consumers of a list
//! or of a generator expression.

use indoc::indoc;

use super::{same_as_cpython, stops_at_the_int_limit, unsupported};

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
