//! The special methods of a class of the program, which the operators and
//! the builtins call.

use indoc::indoc;

use super::{same_as_cpython, unsupported};

/// `==` calls the `__eq__` of the left object, and `!=` its `__ne__`,
/// which by default is the `not` of `__eq__`. A value without `__eq__` on
/// the left gives the call to the `__eq__` of the right object.
#[test]
fn eq_and_ne_call_the_special_methods() {
    same_as_cpython(
        "eq_and_ne_call_the_special_methods",
        indoc! {"
            from dataclasses import dataclass

            class Ponto:
                x: int
                y: int

                def __init__(self, x: int, y: int) -> None:
                    self.x = x
                    self.y = y

                def __eq__(self, other: object) -> bool:
                    print('eq', self.x)
                    if not isinstance(other, Ponto):
                        return False
                    return self.x == other.x and self.y == other.y

            class Sempre:
                def __ne__(self, other: object) -> bool:
                    print('ne')
                    return False

            class Nada:
                pass

            @dataclass
            class Caixa:
                p: Ponto
                n: int

            def valor(n: int) -> object:
                if n > 0:
                    return Ponto(n, n)
                return n

            a = Ponto(1, 2)
            b = Ponto(1, 2)
            c = Ponto(2, 2)
            print(a == b, a != b, a == c, a != c, a == 1, a != 'x')
            n: Ponto | None = None
            print(1 == a, n == a, a == n, Nada() == a, a != Nada())
            s = Sempre()
            print(s != s, s == s, s != Nada(), s == Nada())
            print(Caixa(a, 1) == Caixa(b, 1), Caixa(a, 1) == Caixa(c, 1))
            print(Caixa(a, 1) != Caixa(b, 2))
            print(valor(1) == valor(1), valor(0) == valor(0), valor(1) == a, valor(0) != a)
            n = Ponto(1, 2)
            print(n == a, a != n)
        "},
    );
}

/// The method of the class of the left object runs, which a subclass may
/// override.
#[test]
fn eq_dispatches_on_the_class_of_the_left_object() {
    same_as_cpython(
        "eq_dispatches_on_the_class_of_the_left_object",
        indoc! {"
            class Forma:
                lado: int

                def __init__(self, lado: int) -> None:
                    self.lado = lado

                def __eq__(self, other: object) -> bool:
                    print('forma')
                    return isinstance(other, str) and str(self.lado) == other

            class Quadrado(Forma):
                def __eq__(self, other: object) -> bool:
                    print('quadrado')
                    return isinstance(other, Forma) and self.lado == other.lado

            def igual(a: Forma, b: str) -> bool:
                return a == b

            print(igual(Forma(1), '1'), igual(Quadrado(1), '1'), Forma(1) != '2')
            print(Quadrado(1) == Forma(1), Quadrado(2) != Forma(1))
        "},
    );
}

/// The traceback has the frame of `__eq__` after the frame of the line of
/// the comparison.
#[test]
fn an_exception_in_eq_has_the_frame_of_the_method() {
    same_as_cpython(
        "an_exception_in_eq_has_the_frame_of_the_method",
        indoc! {"
            class Ponto:
                x: int

                def __init__(self, x: int) -> None:
                    self.x = x

                def __eq__(self, other: object) -> bool:
                    return 1 // self.x == 0

            def valor(n: int) -> object:
                if n > 0:
                    return Ponto(n)
                return n

            print(Ponto(1) == Ponto(2))
            print(1 == valor(1))
            print(Ponto(0) != 3)
        "},
    );
}

/// `in`, `count`, `index`, `remove` and `==` of lists take an item that is
/// the object itself as equal, without a call of `__eq__`.
#[test]
fn a_list_tests_the_identity_before_eq() {
    same_as_cpython(
        "a_list_tests_the_identity_before_eq",
        indoc! {"
            class P:
                x: int

                def __init__(self, x: int) -> None:
                    self.x = x

                def __eq__(self, other: object) -> bool:
                    print('eq', self.x)
                    return False

            a = P(1)
            b = P(2)
            print(a in [a], [a].count(a), [b, a].index(a), [a] == [a], [a] != [a])
            xs = [b, a]
            xs.remove(a)
            print(len(xs), a in [b], [a].count(b))
            ys: list[P | int] = [a, 1]
            print(a in ys, 1 in ys, ys.count(a), ys == ys)
            zs = [[a]]
            print([a] in zs, zs == [[a]])
            xs.remove(a)
        "},
    );
}

/// `<` calls the `__lt__` of the left object, or else the `__gt__` of the
/// right object, and a chain compares each pair once, up to the first
/// pair that is false.
#[test]
fn the_order_operators_call_the_special_methods() {
    same_as_cpython(
        "the_order_operators_call_the_special_methods",
        indoc! {"
            class Nota:
                valor: int

                def __init__(self, valor: int) -> None:
                    self.valor = valor

                def __lt__(self, other: 'Nota') -> bool:
                    print('lt', self.valor, other.valor)
                    return self.valor < other.valor

                def __le__(self, other: 'Nota') -> bool:
                    print('le', self.valor, other.valor)
                    return self.valor <= other.valor

                def __eq__(self, other: object) -> bool:
                    print('eq', self.valor)
                    return isinstance(other, Nota) and self.valor == other.valor

            class Peso:
                kg: int

                def __init__(self, kg: int) -> None:
                    self.kg = kg

                def __gt__(self, other: int) -> bool:
                    print('gt', self.kg)
                    return self.kg > other

            def nota(n: int) -> Nota:
                print('nota', n)
                return Nota(n)

            a = Nota(1)
            b = Nota(2)
            print(a < b, b < a, a <= b, a > b, a >= b)
            print(nota(1) < nota(2) < nota(3), nota(3) < nota(2) < nota(1))
            print(nota(1) <= nota(1) == nota(1), nota(1) < nota(2) != nota(3))
            print(Peso(3) > 2, 2 < Peso(3), 5 < Peso(3))
            p = Nota(5)
            print(a < p == p, b <= p < a)
        "},
    );
}

/// The traceback has the frame of the method after the frame of the line
/// of the comparison, in a chain too.
#[test]
fn an_exception_in_lt_has_the_frame_of_the_method() {
    same_as_cpython(
        "an_exception_in_lt_has_the_frame_of_the_method",
        indoc! {"
            class Nota:
                valor: int

                def __init__(self, valor: int) -> None:
                    self.valor = valor

                def __lt__(self, other: 'Nota') -> bool:
                    return 1 // self.valor < other.valor

            print(Nota(1) < Nota(2) < Nota(0) < Nota(3))
        "},
    );
}

/// CPython calls the reflected method of the right object first when its
/// class is a proper subclass of the class of the left object, even when
/// the subclass takes the method from its base.
#[test]
fn a_comparison_calls_the_method_of_a_subclass_on_the_right_first() {
    same_as_cpython(
        "a_comparison_calls_the_method_of_a_subclass_on_the_right_first",
        indoc! {"
            class B:
                n: int

                def __init__(self, n: int) -> None:
                    self.n = n

                def __eq__(self, other: object) -> bool:
                    print('B.eq', self.n)
                    return isinstance(other, B) and self.n == other.n

                def __lt__(self, other: 'B') -> bool:
                    print('B.lt', self.n)
                    return self.n < other.n

                def __gt__(self, other: 'B') -> bool:
                    print('B.gt', self.n)
                    return self.n > other.n


            class D(B):
                def __gt__(self, other: 'B') -> bool:
                    print('D.gt', self.n)
                    return self.n > other.n


            class E(D):
                pass


            def compara(x: B, y: B) -> None:
                print(x == y, x != y, x < y, x > y)


            b = B(1)
            d = D(2)
            e = E(3)
            print(b == d, b < d, d == b, d < b, b != d)
            print(d == e, d < e, e < d, b < e)
            compara(b, d)
            compara(d, b)
            compara(d, e)
            compara(e, e)
            compara(b, b)
        "},
    );
}

/// An arithmetic operator calls the method of the left object, or else
/// the reflected method of the right object, which comes first for a
/// subclass with a reflected method of its own.
#[test]
fn the_arithmetic_operators_call_the_special_methods() {
    same_as_cpython(
        "the_arithmetic_operators_call_the_special_methods",
        indoc! {"
            class V:
                x: int

                def __init__(self, x: int) -> None:
                    self.x = x

                def __add__(self, other: 'V') -> 'V':
                    print('add', self.x, other.x)
                    return V(self.x + other.x)

                def __sub__(self, other: 'V') -> 'V':
                    return V(self.x - other.x)

                def __mul__(self, k: int) -> 'V':
                    return V(self.x * k)

                def __rmul__(self, k: int) -> 'V':
                    print('rmul', self.x)
                    return V(self.x * k)

                def __truediv__(self, k: int) -> float:
                    return self.x / k

                def __floordiv__(self, k: int) -> int:
                    return self.x // k

                def __mod__(self, k: int) -> int:
                    return self.x % k

                def __pow__(self, k: int) -> int:
                    return self.x * k * 10

                def __neg__(self) -> 'V':
                    return V(-self.x)

                def __radd__(self, other: object) -> 'V':
                    print('radd', self.x)
                    if isinstance(other, V):
                        return V(self.x + other.x)
                    return V(self.x)

                def __repr__(self) -> str:
                    return 'V(' + str(self.x) + ')'


            class W(V):
                def __radd__(self, other: object) -> 'V':
                    print('W.radd', self.x)
                    return V(-self.x)


            class U(V):
                pass


            a = V(1)
            b = V(2)
            print(a + b, b - a, a * 3, 3 * a, a / 2, b // 2, b % 2, b ** 3, -a)
            print(1 + a, 1 + W(5), a + W(5), a + U(7))
            print(a + b + V(10), -(a + b))
        "},
    );
}

/// The traceback has the frame of the method after the frame of the line
/// of the operator.
#[test]
fn an_exception_in_add_has_the_frame_of_the_method() {
    same_as_cpython(
        "an_exception_in_add_has_the_frame_of_the_method",
        indoc! {"
            class Nota:
                valor: int

                def __init__(self, valor: int) -> None:
                    self.valor = valor

                def __radd__(self, other: int) -> int:
                    return other // self.valor

            print(1 + Nota(1))
            print(2 + Nota(0))
        "},
    );
}

#[test]
fn eq_outside_the_accepted_form_is_unsupported() {
    for (source, what) in [
        (
            indoc! {"
                class P:
                    def __eq__(self, other: object = None) -> bool:
                        return True
            "},
            "an __eq__ or __ne__ other than (self, other: object) -> bool",
        ),
        (
            indoc! {"
                class P:
                    def __lt__(self, other: int) -> bool:
                        return True

                p = P()
                print(p < 1 < 2)
            "},
            "a chain of comparisons of objects and other values",
        ),
        (
            indoc! {"
                class P:
                    def __lt__(self, other: 'P', x: int = 0) -> bool:
                        return True
            "},
            "a method of an order other than (self, other) -> bool",
        ),
        (
            indoc! {"
                class P:
                    def __add__(self, other: int, x: int = 0) -> int:
                        return 1
            "},
            "a method of an operator other than (self, other)",
        ),
        (
            indoc! {"
                class P:
                    def __matmul__(self, other: int) -> int:
                        return 1
            "},
            "this special method",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                class P:
                    def __eq__(self, other: object) -> bool:
                        return True
            "},
            "a special method in a dataclass",
        ),
    ] {
        assert_eq!(unsupported(source), what, "{source}");
    }
}
