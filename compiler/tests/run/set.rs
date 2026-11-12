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
