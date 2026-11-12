//! The values of a union of types.

use indoc::indoc;

use super::{same_as_cpython, unsupported};

#[test]
fn a_str_or_none_goes_through_variables_parameters_and_results() {
    same_as_cpython(
        "a_str_or_none_goes_through_variables_parameters_and_results",
        indoc! {"
            def pick(flag: bool) -> str | None:
                if flag:
                    return 'a'
                return None


            def keep(s: str | None = None) -> str | None:
                t = s
                return t


            def nothing() -> str | None:
                return


            x: str | None = None
            x = pick(True)
            y = keep(x)
            z = keep()
            w: str | None = 'b'
            w = nothing()
            w = keep(pick(False))
            print('done')
        "},
    );
}

#[test]
fn an_assignment_gives_each_target_the_value_in_its_rep() {
    same_as_cpython(
        "an_assignment_gives_each_target_the_value_in_its_rep",
        indoc! {"
            def twice() -> None:
                a: int | None = None
                b: int = 0
                c: str | None = None
                d: str = ''
                a = b = 7
                c = d = 'x'
                print(a, b, c, d)


            x: int = 0
            y: int | None = None
            z: int | str = 0
            x = y = z = 5
            print(x, y, z)
            u: str | None = 'u'
            v: int | str | None = 0
            u = v = None
            print(u, v)
            twice()
        "},
    );
}

#[test]
fn a_value_of_any_union_goes_through_variables_parameters_and_results() {
    same_as_cpython(
        "a_value_of_any_union_goes_through_variables_parameters_and_results",
        indoc! {"
            def number(flag: bool) -> int | float:
                if flag:
                    return 1
                return 2.5


            def keep(x: int | float | bool | str | None = None) -> int | float | bool | str | None:
                y = x
                return y


            def maybe(s: str | None) -> str | int | None:
                return s


            n: int | None = None
            n = 3
            m: int | float = number(True)
            m = number(False)
            m = 7
            v = keep(m)
            v = keep(True)
            v = keep(False)
            v = keep('a')
            v = keep(-1073741824)
            v = keep(1073741823)
            v = keep()
            w = maybe('b')
            w = maybe(None)
            b: bool | int = True
            b = 0
            print('done')
        "},
    );
}

#[test]
fn is_none_tests_a_value_and_narrows_a_union() {
    same_as_cpython(
        "is_none_tests_a_value_and_narrows_a_union",
        indoc! {"
            def describe(s: str | None) -> str:
                if s is None:
                    return 'nothing'
                return s.upper()


            def next_of(n: int | None) -> int:
                if n is not None:
                    return n + 1
                return 0


            def half(x: float | None) -> float:
                if None is x:
                    return 0.0
                return x / 2


            def flip(b: bool | None) -> bool:
                if b is None:
                    return False
                return not b


            def first(s: str | int | None) -> bool:
                return s is None


            print(describe('abc'), describe(None))
            print(next_of(-1073741824), next_of(41), next_of(None))
            print(half(5.0), half(None))
            print(flip(True), flip(False), flip(None))
            print(first(None), first(3), first('a'))
            n = 1
            t = 'a'
            print(n is None, t is not None, None is None, None is not None)
            g: str | None = None
            if g is None:
                g = 'x'
            print(g.upper())
        "},
    );
}

#[test]
fn print_writes_a_value_of_a_union_as_its_type_says() {
    same_as_cpython(
        "print_writes_a_value_of_a_union_as_its_type_says",
        indoc! {"
            def show(x: int | float | bool | str | None) -> None:
                print(x)


            def maybe(s: str | None) -> None:
                print('<', s, '>')


            show(None)
            show(-1073741824)
            show(1073741823)
            show(0.1)
            show(-0.0)
            show(1e16)
            show(1e999 - 1e999)
            show(True)
            show(False)
            show('')
            show('é😀')
            maybe(None)
            maybe('a')
            n: int | None = None
            f: float | None = 2.5
            print(n, f, n, f)
            print(None, maybe('b'), None)
        "},
    );
}

#[test]
fn a_value_of_a_union_compares_as_its_type_does() {
    same_as_cpython(
        "a_value_of_a_union_compares_as_its_type_does",
        indoc! {"
            def same(a: int | float | bool | str | None, b: int | float | bool | str | None) -> None:
                print(a == b, a != b)


            def order(a: int | float | bool, b: int | float | bool) -> None:
                print(a < b, a <= b, a > b, a >= b)


            def words(a: str | None, b: str) -> None:
                if a is not None:
                    print(a < b, a <= b, a > b, a >= b)
                print(a == b, a != b, b == a)


            nan = 1e999 - 1e999
            same(None, None)
            same(None, 0)
            same(0, None)
            same('a', None)
            same(1, 1.0)
            same(1, True)
            same(0, False)
            same(2, 2.5)
            same('1', 1)
            same(1, '1')
            same('ab', 'ab')
            same('ab', 'ac')
            same(nan, nan)
            same(0.0, -0.0)
            order(1, 2)
            order(2, 1)
            order(2, 2.0)
            order(True, 1)
            order(False, 0.5)
            order(-1073741824, 1073741823)
            order(nan, 1)
            order(1, nan)
            words('a', 'b')
            words('b', 'a')
            words('a', 'a')
            words(None, 'a')
            x: int | None = 3
            print(1 < x < 5, 1 < x < 2, x == 3 == x)
        "},
    );
}

#[test]
fn a_use_of_a_str_or_none_that_the_compiler_lacks_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            def f(s: str | None) -> str:
                return str(s)
        "}),
        "str of a union"
    );
    assert_eq!(
        unsupported(indoc! {"
            def f(s: str | None) -> str:
                return s or 'a'
        "}),
        "the truth of a union"
    );
    assert_eq!(
        unsupported(indoc! {"
            def f(self: str | None) -> None:
                pass
        "}),
        "a variable self of a union"
    );
    assert_eq!(
        unsupported(indoc! {"
            g: str | None = None


            def f() -> None:
                global g
                g = None


            if g is not None:
                f()
                print(g.upper())
        "}),
        "a read of a global of a union that a test narrows"
    );
    assert_eq!(
        unsupported(indoc! {"
            def f(self: int | None) -> None:
                pass
        "}),
        "a variable self of a union"
    );
}

#[test]
fn a_test_of_equality_keeps_the_type_of_a_narrowed_value() {
    same_as_cpython(
        "a_test_of_equality_keeps_the_type_of_a_narrowed_value",
        indoc! {"
            def step_of(step: int | None) -> int:
                if step is None:
                    return 1
                if step == 0:
                    raise ValueError('slice step cannot be zero')
                return step


            def name_of(s: str | None) -> str:
                if s is None or s == '':
                    return '?'
                return s


            def first(s: str) -> str:
                if s != '':
                    return s[0]
                return s


            print(step_of(None), step_of(3), name_of(None), name_of(''), name_of('a'))
            print(first('xy'), first(''))
            print(step_of(0))
        "},
    );
}

#[test]
fn a_value_of_object_holds_any_value_and_writes_it() {
    same_as_cpython(
        "a_value_of_object_holds_any_value_and_writes_it",
        indoc! {"
            from dataclasses import dataclass
            from enum import Enum, auto


            @dataclass
            class Ponto:
                x: int
                y: int


            class Cor(Enum):
                AZUL = auto()


            def mostra(x: object) -> object:
                print(x, repr(x))
                if x is None:
                    print('nada', x)
                return x


            v: object = mostra(1)
            print(v == 1, v != 1.0, v == '1')
            v = mostra('a')
            print(v == 'a')
            v = mostra(2.5)
            v = mostra(True)
            v = mostra(None)
            print(v is None)
            v = mostra(Ponto(1, 2))
            print(v == Ponto(1, 2), v == Ponto(2, 1), v == 3)
            v = mostra(Cor.AZUL)
            print(v == Cor.AZUL)
        "},
    );
}

#[test]
fn a_list_as_an_object_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            def f(x: object) -> None:
                print(x)


            f([1, 2])
        "}),
        "an argument of this type"
    );
}
