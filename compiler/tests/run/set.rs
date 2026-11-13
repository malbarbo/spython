//! The values of `set`.

use indoc::indoc;

use super::{same_as_cpython, unsupported};

/// The table grows and keeps the slots of the removed items as CPython
/// does, so `len` and `in` agree after many changes.
#[test]
fn a_set_takes_and_loses_items() {
    same_as_cpython(
        "a_set_takes_and_loses_items",
        indoc! {"
            def conta(s: set[int], n: int) -> int:
                total = 0
                for i in range(n):
                    if i in s:
                        total += 1
                return total

            s = {1, 2, 3, 2, 1}
            print(len(s), 2 in s, 4 in s, 4 not in s)
            s.add(4)
            s.add(4)
            s.remove(1)
            s.discard(1)
            s.discard(10)
            print(len(s), 1 in s, 4 in s)
            vazio: set[int] = set()
            print(len(vazio), 0 in vazio)
            for i in range(2000):
                vazio.add(i * 7)
            for i in range(0, 2000, 3):
                vazio.remove(i * 7)
            for i in range(100):
                vazio.add(-i)
            print(len(vazio), conta(vazio, 14000))
            vazio.clear()
            print(len(vazio), 7 in vazio)
            vazio.add(7)
            print(len(vazio), 7 in vazio)
        "},
    );
}

#[test]
fn a_set_of_str_float_and_bool_runs() {
    same_as_cpython(
        "a_set_of_str_float_and_bool_runs",
        indoc! {"
            nomes = {'ana', 'bia', 'caio', 'ana'}
            print(len(nomes), 'bia' in nomes, 'davi' in nomes)
            for i in range(500):
                nomes.add('n' + str(i))
            for i in range(0, 500, 2):
                nomes.discard('n' + str(i))
            print(len(nomes), 'n3' in nomes, 'n4' in nomes)
            xs = {1.5, 2.0, -0.0}
            print(len(xs), 0.0 in xs, 2.0 in xs, 2.5 in xs)
            bs = {True, False, True}
            print(len(bs), True in bs)
            xs.add(0.0)
            xs.add(-1.5)
            print(len(xs), -1.5 in xs)
        "},
    );
}

/// The item of the `set` is on the left of `==`, and `in`, `add` and
/// `remove` compare an item with the same `hash` alone.
#[test]
fn a_set_compares_items_as_cpython_does() {
    same_as_cpython(
        "a_set_compares_items_as_cpython_does",
        indoc! {"
            class Chave:
                nome: str
                h: int

                def __init__(self, nome: str, h: int) -> None:
                    self.nome = nome
                    self.h = h

                def __eq__(self, other: object) -> bool:
                    if not isinstance(other, Chave):
                        return False
                    print('eq', self.nome, other.nome)
                    return self.h == other.h

                def __hash__(self) -> int:
                    print('hash', self.nome)
                    return self.h % 3

            a = Chave('a', 1)
            s = {a, Chave('b', 4), Chave('c', 7)}
            print(len(s))
            print(Chave('d', 4) in s)
            s.add(Chave('e', 7))
            s.remove(Chave('f', 1))
            print(len(s), a in s)
        "},
    );
}

#[test]
fn a_remove_of_a_missing_item_raises_key_error() {
    same_as_cpython(
        "a_remove_of_a_missing_item_raises_key_error",
        indoc! {"
            s = {'a', 'b'}
            s.remove('a')
            s.remove(\"it's\")
        "},
    );
}

#[test]
fn a_set_literal_with_a_starred_item_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            a = {1, 2}
            b = {*a, 3}
        "}),
        "a starred item"
    );
}

/// A comparison that removes the item of the slot or moves the items to
/// a new table starts the search again, as `set_lookkey` and
/// `set_add_entry` do.
#[test]
fn a_comparison_that_changes_the_set_starts_the_search_again() {
    same_as_cpython(
        "a_comparison_that_changes_the_set_starts_the_search_again",
        indoc! {"
            class K:
                n: int

                def __init__(self, n: int) -> None:
                    self.n = n

                def __eq__(self, other: object) -> bool:
                    if not isinstance(other, K):
                        return False
                    if acao[0] == 1:
                        acao[0] = 0
                        print('remove', self.n)
                        s.discard(self)
                    elif acao[0] == 2:
                        acao[0] = 0
                        print('cresce', self.n)
                        for i in range(10):
                            s.add(K(100 + i))
                    return self.n == other.n

                def __hash__(self) -> int:
                    return self.n % 2

            acao = [0]
            s: set[K] = set()
            a = K(1)
            b = K(3)
            c = K(5)
            s.add(a)
            s.add(b)
            s.add(c)
            acao[0] = 1
            print(K(7) in s, len(s), a in s)
            s.add(a)
            acao[0] = 1
            s.add(K(9))
            print(len(s), a in s)
            acao[0] = 2
            print(K(3) in s, len(s))
            acao[0] = 2
            s.add(K(11))
            print(len(s))
            acao[0] = 1
            s.remove(K(5))
            print(len(s), c in s)
        "},
    );
}

#[test]
fn print_writes_a_set_as_cpython_does() {
    same_as_cpython(
        "print_writes_a_set_as_cpython_does",
        indoc! {"
            vazio: set[int] = set()
            print(vazio, repr(vazio), str(vazio))
            s = {3, 1, 2, -5, 100, 8}
            print(s)
            nomes: set[str] = set()
            for nome in ['ana', \"it's\", 'bia', 'caio', 'davi', 'eva']:
                nomes.add(nome)
            print(nomes, repr(nomes) == str(nomes))
            for i in range(30):
                nomes.add('n' + str(i))
            nomes.discard('ana')
            print(nomes)
            print([{1.5, -0.0}], {'k': {True}})
            xs: set[int | str | None] = {1, 'a', None}
            print(xs)
        "},
    );
}

/// A `set` inside its own `repr` is `set(...)`, through an item whose
/// `repr` writes the `set`.
#[test]
fn a_set_inside_its_own_repr_is_written_with_dots() {
    same_as_cpython(
        "a_set_inside_its_own_repr_is_written_with_dots",
        indoc! {"
            class N:
                def __eq__(self, other: object) -> bool:
                    return isinstance(other, N)

                def __hash__(self) -> int:
                    return 1

                def __repr__(self) -> str:
                    return 'N' + repr(s)

            s: set[N] = set()
            s.add(N())
            print(s)
        "},
    );
}

#[test]
fn a_for_over_a_set_takes_the_items_in_the_order_of_cpython() {
    same_as_cpython(
        "a_for_over_a_set_takes_the_items_in_the_order_of_cpython",
        indoc! {"
            def soma(s: set[int]) -> int:
                total = 0
                for x in s:
                    total += x
                return total

            s: set[int] = set()
            for x in [5, 3, 40, -2, 17]:
                s.add(x)
            for x in s:
                print(x)
            print(soma(s))
            nomes: set[str] = set()
            for i in range(40):
                nomes.add('n' + str(i * 3))
            for i in range(10):
                nomes.discard('n' + str(i * 6))
            ordem = []
            for nome in nomes:
                ordem.append(nome)
            print(ordem)
            for x in s:
                if x > 10:
                    print('achou', x)
                    break
            else:
                print('nada')
            for x in {1, 2}:
                for y in {1, 2}:
                    print(x, y)
            else:
                print('fim')
            vazio: set[float] = set()
            for v in vazio:
                print(v)
        "},
    );
}

/// The loop sees each slot in order, so an item that the body puts in a
/// later slot comes in the loop when the size stays.
#[test]
fn a_for_over_a_set_that_keeps_its_size_goes_on() {
    same_as_cpython(
        "a_for_over_a_set_that_keeps_its_size_goes_on",
        indoc! {"
            s: set[int] = {1, 2, 3}
            for x in s:
                print(x)
                if x < 6:
                    s.discard(x)
                    s.add(x + 3)
            print(s)
        "},
    );
}

#[test]
fn a_for_over_a_set_that_changes_size_raises_runtime_error() {
    same_as_cpython(
        "a_for_over_a_set_that_changes_size_raises_runtime_error",
        indoc! {"
            s: set[int] = {1, 2, 3}
            for x in s:
                print(x)
                s.add(x + 10)
        "},
    );
}

/// A copy of a `set` after removals puts the items in a new table, in the
/// order of the slots, so it may come out in another order.
#[test]
fn copy_and_set_of_a_set_give_a_new_set() {
    same_as_cpython(
        "copy_and_set_of_a_set_give_a_new_set",
        indoc! {"
            s: set[str] = set()
            for i in range(20):
                s.add('k' + str(i))
            for i in range(0, 20, 3):
                s.remove('k' + str(i))
            t = s.copy()
            u = set(s)
            print(s)
            print(t)
            print(u)
            t.add('novo')
            print(len(s), len(t), len(u), 'novo' in s)
            vazio: set[int] = set()
            print(vazio.copy(), set(vazio))
        "},
    );
}

#[test]
fn a_set_of_a_list_takes_its_items_in_order() {
    same_as_cpython(
        "a_set_of_a_list_takes_its_items_in_order",
        indoc! {"
            xs = [3, 1, 3, 2, 17, 9, 1, 25]
            print(set(xs))
            nomes = ['bia', 'ana', 'bia', 'caio', 'davi', 'eva', 'ana']
            s = set(nomes)
            print(s, len(s))
            vazia: list[str] = []
            print(set(vazia))
            notas: list[float] = [7.5, 2.0, 7.5, -0.0, 0.0]
            print(set(notas))
        "},
    );
}

#[test]
fn the_union_of_sets_runs_as_cpython_does() {
    same_as_cpython(
        "the_union_of_sets_runs_as_cpython_does",
        indoc! {"
            a: set[str] = set()
            b: set[str] = set()
            for i in range(12):
                a.add('a' + str(i))
                b.add('b' + str(i * 2))
                b.add('a' + str(i * 3))
            print(a | b)
            print(b | a)
            print(a | a)
            c = a
            a |= b
            print(len(a), len(c))
            c |= c
            print(len(c))
            vazio: set[str] = set()
            print(vazio | vazio, len(vazio | b))
        "},
    );
}

/// A class of the program may define `|` too.
#[test]
fn a_class_defines_or_and_its_reflected_method() {
    same_as_cpython(
        "a_class_defines_or_and_its_reflected_method",
        indoc! {"
            class Bits:
                v: int

                def __init__(self, v: int) -> None:
                    self.v = v

                def __or__(self, other: 'Bits') -> 'Bits':
                    print('or', self.v, other.v)
                    return Bits(self.v + other.v)

                def __ior__(self, other: 'Bits') -> 'Bits':
                    print('ior', self.v, other.v)
                    self.v = self.v * other.v
                    return self

            a = Bits(2)
            b = Bits(3)
            print((a | b).v)
            a |= b
            print(a.v)
        "},
    );
}

#[test]
fn the_intersection_and_the_differences_of_sets_run_as_cpython_does() {
    same_as_cpython(
        "the_intersection_and_the_differences_of_sets_run_as_cpython_does",
        indoc! {"
            def faz(n: int, m: int, passo: int) -> set[str]:
                s: set[str] = set()
                for i in range(n, m, passo):
                    s.add('x' + str(i))
                return s

            a = faz(0, 30, 1)
            b = faz(10, 60, 2)
            pequeno = faz(5, 8, 1)
            grande = faz(0, 400, 1)
            def mostra(x: set[str], y: set[str]) -> None:
                print(x & y)
                print(x - y)
                print(x ^ y)

            mostra(a, b)
            mostra(b, a)
            mostra(a, pequeno)
            mostra(pequeno, grande)
            mostra(grande, pequeno)
            mostra(a, a)
        "},
    );
}

/// The augmented assignments change the `set` itself, and a `set` with
/// itself on the right loses its items or keeps them, as in CPython.
#[test]
fn the_augmented_operators_of_sets_change_the_set() {
    same_as_cpython(
        "the_augmented_operators_of_sets_change_the_set",
        indoc! {"
            def faz(n: int, m: int) -> set[int]:
                s: set[int] = set()
                for i in range(n, m):
                    s.add(i * 37 % 101)
                return s

            a = faz(0, 40)
            b = a
            a &= faz(20, 90)
            print(a, len(b))
            a -= faz(0, 5)
            print(a, len(b))
            a ^= faz(30, 50)
            print(a, len(b))
            a -= faz(0, 900)
            print(a, len(b))
            c = faz(0, 10)
            c ^= c
            print(c)
            c = faz(0, 10)
            c &= c
            print(c)
            c -= c
            print(c)
        "},
    );
}

#[test]
fn the_comparisons_of_sets_run_as_cpython_does() {
    same_as_cpython(
        "the_comparisons_of_sets_run_as_cpython_does",
        indoc! {"
            def faz(xs: list[str]) -> set[str]:
                s: set[str] = set()
                for x in xs:
                    s.add(x)
                return s

            a = faz(['a', 'b'])
            b = faz(['b', 'a', 'c'])
            c = faz(['c', 'b', 'a'])
            vazio: set[str] = set()
            def compara(x: set[str], y: set[str]) -> None:
                print(x == y, x != y, x < y, x <= y, x > y, x >= y)

            compara(a, b)
            compara(b, a)
            compara(b, c)
            compara(a, a)
            compara(vazio, a)
            compara(a, vazio)
            print(a < b < c, a <= b <= c)
            o: object = a
            print(o == a, a == o, o == b, o == 1)
            print({1, 2} == {2, 1}, {1.5} == {1.5}, {True} == {False})
        "},
    );
}

/// The item of the `set` on the left is on the left of `==` when a
/// comparison of `set`s looks for it in the other one.
#[test]
fn the_comparison_of_sets_compares_items_as_cpython_does() {
    same_as_cpython(
        "the_comparison_of_sets_compares_items_as_cpython_does",
        indoc! {"
            class K:
                n: int

                def __init__(self, n: int) -> None:
                    self.n = n

                def __eq__(self, other: object) -> bool:
                    if not isinstance(other, K):
                        return False
                    print('eq', self.n, other.n)
                    return self.n % 10 == other.n % 10

                def __hash__(self) -> int:
                    return self.n % 10

            a: set[K] = set()
            b: set[K] = set()
            for i in range(4):
                a.add(K(i))
                b.add(K(i + 10))
            print(a == b)
            print(a <= b)
            print(len(a & b), len(a - b), len(a ^ b), len(a | b))
        "},
    );
}

#[test]
fn a_comparison_of_sets_of_two_types_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            def f(a: set[int], b: set[str]) -> bool:
                return a == b
        "}),
        "a comparison of dicts or sets of two types"
    );
}
