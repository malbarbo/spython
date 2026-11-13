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

#[test]
fn a_lambda_is_a_function_with_the_frame_lambda() {
    same_as_cpython(
        "a_lambda_is_a_function_with_the_frame_lambda",
        indoc! {"
            from typing import Callable

            def aplica(f: Callable[[int], int], v: int) -> int:
                return f(v)

            base = 10
            dobro: Callable[[int], int] = lambda x: 2 * x
            print(dobro(4), aplica(lambda y: y + base, 5), (lambda: 'oi')())
            fs: list[Callable[[int], int]] = []
            i = 0
            while i < 3:
                fs.append(lambda n: n * n)
                i += 1
            print(fs[0] == fs[1], fs[0] == fs[0], fs[0](3), fs[2](4))
            diz: Callable[[str], None] = lambda s: print('diz', s)
            diz('a')
            quebra: Callable[[int], int] = lambda n: 10 // n
            print(aplica(quebra, 0))
        "},
    );
}

#[test]
fn a_lambda_that_reads_a_global_before_its_assignment_raises_name_error() {
    same_as_cpython(
        "a_lambda_that_reads_a_global_before_its_assignment_raises_name_error",
        indoc! {"
            from typing import Callable

            f: Callable[[], int] = lambda: depois + 1
            print(f())
            depois = 1
        "},
    );
}

#[test]
fn a_lambda_captures_the_variables_of_its_function() {
    same_as_cpython(
        "a_lambda_captures_the_variables_of_its_function",
        indoc! {"
            from dataclasses import dataclass
            from typing import Callable

            @dataclass
            class Conta:
                saldo: float

                def depositador(self) -> Callable[[float], float]:
                    return lambda v: self.saldo + v

            def soma_de(k: int) -> Callable[[int], int]:
                return lambda x: x + k

            def aninhado(a: int) -> Callable[[int], Callable[[int], int]]:
                nome = 'x'
                return lambda b: lambda c: a + b + c + len(nome)

            def opcional(s: str | None) -> Callable[[], str]:
                return lambda: 'nada' if s is None else s

            print(soma_de(3)(4), aninhado(1)(2)(3))
            print(Conta(1.5).depositador()(2.0))
            print(opcional(None)(), opcional('a')())
        "},
    );
}

/// A variable that changes after the capture lives in a cell, and the
/// function sees its last value, as in CPython.
#[test]
fn a_lambda_sees_the_last_value_of_a_variable_that_changes() {
    same_as_cpython(
        "a_lambda_sees_the_last_value_of_a_variable_that_changes",
        indoc! {"
            from typing import Callable

            def lacos() -> None:
                fs: list[Callable[[], int]] = []
                i = 0
                while i < 3:
                    fs.append(lambda: i)
                    i += 1
                print(fs[0](), fs[1](), fs[2]())
                gs: list[Callable[[], int]] = []
                for j in range(3):
                    gs.append(lambda: j * 10)
                print(gs[0](), gs[2]())

            def muda() -> None:
                texto = 'a'
                g: Callable[[], str] = lambda: texto + '!'
                print(g())
                texto = 'b'
                print(g())

            def param_muda(n: int) -> Callable[[], int]:
                g: Callable[[], int] = lambda: n * 2
                n = n + 1
                return g

            lacos()
            muda()
            print(param_muda(5)())
        "},
    );
}

#[test]
fn a_lambda_that_reads_a_variable_without_a_value_raises_name_error() {
    same_as_cpython(
        "a_lambda_that_reads_a_variable_without_a_value_raises_name_error",
        indoc! {"
            from typing import Callable

            def condicional(b: bool) -> Callable[[], int]:
                if b:
                    x = 1
                return lambda: x

            print(condicional(True)())
            valr = 3

            def antes() -> None:
                g: Callable[[], int] = lambda: valor + 1
                print(g())
                valor = 1

            antes()
        "},
    );
}

/// The suggestion of a `NameError` in the code of a `lambda` looks at the
/// variables that the `lambda` captures, after its parameters.
#[test]
fn a_name_error_in_a_lambda_suggests_a_captured_variable() {
    same_as_cpython(
        "a_name_error_in_a_lambda_suggests_a_captured_variable",
        indoc! {"
            from typing import Callable

            def f() -> None:
                zeta = 1
                contador = 1
                g: Callable[[int], int] = lambda contadx: contadr + contador + zeta
                print(g(1))

            f()
            contadr = 5
        "},
    );
}

#[test]
fn a_def_inside_a_function_is_a_value_of_the_function() {
    same_as_cpython(
        "a_def_inside_a_function_is_a_value_of_the_function",
        indoc! {"
            from dataclasses import dataclass
            from typing import Callable

            contagem = 0

            def fatorial(n: int) -> int:
                def fat(k: int) -> int:
                    if k <= 1:
                        return 1
                    return k * fat(k - 1)
                return fat(n)

            def contador() -> Callable[[], int]:
                total = 0
                def proximo() -> int:
                    return total + 1
                total = 10
                return proximo

            def mutuos(n: int) -> bool:
                def par(k: int) -> bool:
                    if k == 0:
                        return True
                    return impar(k - 1)
                def impar(k: int) -> bool:
                    if k == 0:
                        return False
                    return par(k - 1)
                return par(n)

            def aplica(xs: list[int]) -> list[int]:
                fator = 3
                def vezes(x: int) -> int:
                    def interno() -> int:
                        return x * fator
                    return interno()
                ys: list[int] = []
                for x in xs:
                    ys.append(vezes(x))
                return ys

            def escolhe(b: bool) -> int:
                def conta() -> int:
                    global contagem
                    contagem += 1
                    return contagem
                conta()
                if b:
                    def h() -> int:
                        return 1
                else:
                    def h() -> int:
                        return 2
                return conta() + h()

            @dataclass
            class Pilha:
                itens: list[int]

                def empilha_todos(self, xs: list[int]) -> None:
                    def um(x: int) -> None:
                        self.itens.append(x)
                    for x in xs:
                        um(x)

            p = Pilha([])
            p.empilha_todos([1, 2, 3])
            print(fatorial(5), contador()(), mutuos(7), aplica([1, 2]))
            print(escolhe(True), escolhe(False), contagem, p.itens)
        "},
    );
}

#[test]
fn a_def_inside_a_function_has_a_frame_of_its_name() {
    same_as_cpython(
        "a_def_inside_a_function_has_a_frame_of_its_name",
        indoc! {"
            def quebra() -> None:
                def divide(n: int) -> int:
                    return 10 // n
                print(divide(0))

            quebra()
        "},
    );
}

/// A function that calls a function defined after it in the same function
/// runs once both have a value, and raises the `NameError` of a free
/// variable before.
#[test]
fn a_def_that_calls_a_later_def_raises_name_error_before_it() {
    same_as_cpython(
        "a_def_that_calls_a_later_def_raises_name_error_before_it",
        indoc! {"
            def cedo() -> None:
                def a() -> int:
                    return b()
                print(a())
                def b() -> int:
                    return 1

            cedo()
        "},
    );
}

#[test]
fn a_default_of_a_def_inside_a_function_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            def f() -> int:
                def g(n: int = 1) -> int:
                    return n
                return g(2)
        "}),
        "a default of a def inside a function"
    );
}
