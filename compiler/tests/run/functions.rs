//! Functions, their calls, and the globals that they read and assign.

use indoc::{formatdoc, indoc};

use super::{same_as_cpython, unsupported};

#[test]
fn a_function_takes_its_arguments_and_returns_its_result() {
    same_as_cpython(
        "a_function_takes_its_arguments_and_returns_its_result",
        indoc! {"
            def soma(a: int, b: int) -> int:
                total = a + b
                return total

            def maior(a: int, b: int) -> int:
                if a > b:
                    return a
                else:
                    return b

            def eco(s: str, vezes: int) -> None:
                while vezes > 0:
                    print(s)
                    vezes -= 1

            def primeiro(n: int) -> int:
                i = 2
                while True:
                    if n % i == 0:
                        return i
                    i += 1

            def nada(b: bool) -> None:
                if b:
                    return
                print('nada')

            def par(n: int) -> bool:
                return n % 2 == 0

            def texto(b: bool) -> str:
                if b:
                    return 'sim'
                return 'não'

            print(soma(2, 3), maior(soma(1, 1), 7), maior(9, 4))
            eco('oi', 2)
            print(primeiro(91), par(4), texto(par(3)))
            nada(True)
            nada(False)
        "},
    );
}

#[test]
fn a_function_assigns_a_global_that_it_declares() {
    same_as_cpython(
        "a_function_assigns_a_global_that_it_declares",
        indoc! {"
            count = 0
            name = 'a'


            def bump(n: int) -> int:
                global count, name
                count += n
                if count > 2:
                    name = name + 'b'
                return count


            def local() -> None:
                count = 10
                print(count)


            print(bump(1), bump(2), count, name)
            local()
            print(count)
        "},
    );
}

#[test]
fn a_global_that_only_a_function_assigns_has_a_value_after_the_call() {
    same_as_cpython(
        "a_global_that_only_a_function_assigns_has_a_value_after_the_call",
        indoc! {"
            name: str


            def init() -> None:
                global name
                name = 'ana'


            def show() -> None:
                print(name)


            init()
            show()
            print(name)  # ty: ignore[possibly-unresolved-reference]
        "},
    );
}

#[test]
fn a_global_read_before_its_assignment_raises_name_error() {
    same_as_cpython(
        "a_global_read_before_its_assignment_raises_name_error",
        indoc! {"
            def bump() -> None:
                global total
                total += 1


            bump()
            total = 0
        "},
    );
}

#[test]
fn a_global_with_values_of_two_types_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
                x = 1


                def f() -> None:
                    global x
                    x = 'a'  # ty: ignore[invalid-assignment]
            "}),
        "a variable with values of two types"
    );
}

#[test]
fn a_function_reads_the_globals_and_calls_a_function_defined_after_it() {
    same_as_cpython(
        "a_function_reads_the_globals_and_calls_a_function_defined_after_it",
        indoc! {"
            base = 10

            def f(n: int) -> int:
                return g(n) + base

            def g(n: int) -> int:
                return n * fator

            fator = 3
            print(f(2))
            base = 100
            print(f(2))
        "},
    );
}

#[test]
fn an_exception_in_a_function_has_the_frames_of_the_calls() {
    same_as_cpython(
        "an_exception_in_a_function_has_the_frames_of_the_calls",
        indoc! {"
            def k(n: int) -> int:
                return 1 // n

            def g(n: int) -> int:
                return k(n) + 1

            print(g(1))
            print(k(2) + g(0))
        "},
    );
}

#[test]
fn a_read_of_a_local_before_its_assignment_raises_unbound_local_error() {
    same_as_cpython(
        "a_read_of_a_local_before_its_assignment_raises_unbound_local_error",
        indoc! {"
            def f(a: int) -> int:
                if a > 0:
                    x = 1
                return x + 7 // a  # ty: ignore[possibly-unresolved-reference]

            def g(n: int) -> int:
                return f(n) + 1

            print(g(1))
            print(g(0))
        "},
    );
}

#[test]
fn a_read_of_a_global_before_its_assignment_raises_name_error() {
    same_as_cpython(
        "a_read_of_a_global_before_its_assignment_raises_name_error",
        indoc! {"
            def h() -> int:
                return y

            print(h())
            y = 3
        "},
    );
}

#[test]
fn a_call_before_the_def_raises_name_error() {
    same_as_cpython(
        "a_call_before_the_def_raises_name_error",
        indoc! {"
            def g() -> int:
                return f() + 1

            print('antes')
            print(g())

            def f() -> int:
                return 1
        "},
    );
}

#[test]
fn a_call_before_the_def_in_the_module_raises_name_error() {
    same_as_cpython(
        "a_call_before_the_def_in_the_module_raises_name_error",
        indoc! {"
            print(f())  # ty: ignore[unresolved-reference]

            def f() -> int:
                return 1
        "},
    );
}

#[test]
fn a_function_that_may_end_without_a_return_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
                def f(n: int) -> int:  # ty: ignore[invalid-return-type]
                    if n > 0:
                        return 1
            "}),
        "a function that may end without a return"
    );
    assert_eq!(
        unsupported(indoc! {"
                def f(n: int) -> int:  # ty: ignore[invalid-return-type]
                    while True:
                        if n > 0:
                            break
                        return 1
            "}),
        "a function that may end without a return"
    );
    assert_eq!(
        unsupported(indoc! {"
                def f(n: int) -> int:  # ty: ignore[invalid-return-type]
                    while True:
                        while n > 0:
                            n = n - 1
                        else:
                            break
            "}),
        "a function that may end without a return"
    );
}

#[test]
fn a_def_outside_what_the_compiler_covers_is_unsupported() {
    let params = (0..1001).map(|i| format!("p{i}: int")).collect::<Vec<_>>();
    assert_eq!(
        unsupported(&formatdoc! {"
            def f({}) -> int:
                return 1
        ", params.join(", ")}),
        "a def with more than 1000 parameters"
    );
    assert_eq!(
        unsupported(indoc! {"
            def f(n: int) -> int:
                return n

            print(f)
        "}),
        "print of a function"
    );
    assert_eq!(
        unsupported(indoc! {"
            def f() -> None:
                def g() -> None:
                    pass
        "}),
        "a def in a function"
    );
    assert_eq!(
        unsupported(indoc! {"
                def f() -> int:
                    return 1

                print(f(n=1))  # ty: ignore[unknown-argument]
            "}),
        "a call with these arguments"
    );
}

#[test]
fn a_call_evaluates_its_keyword_arguments_in_the_order_of_the_call() {
    same_as_cpython(
        "a_call_evaluates_its_keyword_arguments_in_the_order_of_the_call",
        indoc! {"
            def show(a: int, b: str, c: float) -> None:
                print(a, b, c)


            def trace(n: int) -> int:
                print('eval', n)
                return n


            def inner() -> None:
                show(c=1.5, a=trace(4), b='w' * trace(2))


            show(1, c=2.5, b='x')
            show(c=float(trace(1)), b='y', a=trace(2))
            show(b='z', a=trace(3), c=0.5)
            show(trace(5), 'v', c=float(trace(6)))
            inner()
        "},
    );
}

#[test]
fn a_call_gives_a_keyword_only_parameter_by_its_name() {
    same_as_cpython(
        "a_call_gives_a_keyword_only_parameter_by_its_name",
        indoc! {"
            def trace(n: int) -> int:
                print('eval', n)
                return n


            def show(a: int, *, sep: str, times: int) -> None:
                print(str(a) + sep * times)


            def only(*, a: int, b: int) -> int:
                return a - b


            print(only(b=trace(1), a=trace(2)))
            show(trace(3), times=trace(2), sep='-')
        "},
    );
}

#[test]
fn a_def_evaluates_a_default_once_and_a_call_without_the_argument_takes_it() {
    same_as_cpython(
        "a_def_evaluates_a_default_once_and_a_call_without_the_argument_takes_it",
        indoc! {"
            def trace(n: int) -> int:
                print('eval', n)
                return n


            base = 10


            def show(a: int, b: int = trace(base), *, sep: str = '-', end: str = '.') -> None:
                print(str(a) + sep + str(b) + end)


            base = 20
            show(1)
            show(2, 3)
            show(4, end='!')
            show(b=trace(5), a=trace(6), sep='+')
            show(sep=':', a=7)


            def inner() -> None:
                show(8)
                show(9, sep='=')


            inner()
        "},
    );
}

#[test]
fn a_default_that_raises_stops_the_def() {
    same_as_cpython(
        "a_default_that_raises_stops_the_def",
        indoc! {"
            def f(n: int = 1 // 0) -> int:  # ty: ignore[division-by-zero]
                return n


            print(f())
        "},
    );
}

#[test]
fn a_call_with_a_keyword_argument_before_the_def_raises_name_error() {
    same_as_cpython(
        "a_call_with_a_keyword_argument_before_the_def_raises_name_error",
        indoc! {"
            def trace(n: int) -> int:
                print('eval', n)
                return n


            show(b=trace(1), a=trace(2))  # ty: ignore[unresolved-reference]


            def show(a: int, b: int) -> None:
                print(a, b)
        "},
    );
}

#[test]
fn each_call_outside_what_the_compiler_covers_has_its_own_message() {
    assert_eq!(
        unsupported(indoc! {"
            print((lambda: 1)())
        "}),
        "a call of an expression other than a name"
    );
    assert_eq!(
        unsupported(indoc! {"
            def g() -> None:
                pass


            def f() -> None:
                while True:
                    h()  # ty: ignore[possibly-unresolved-reference]
                    h = g
        "}),
        "a call of a local variable"
    );
    assert_eq!(
        unsupported(indoc! {"
            print(hex(65))
        "}),
        "a call of a function that the module does not define"
    );
}

/// A name with a second `def` calls the function of the last `def` that
/// ran, in the code of the module and in a function.
#[test]
fn a_second_def_replaces_the_function_of_the_name() {
    same_as_cpython(
        "a_second_def_replaces_the_function_of_the_name",
        indoc! {"
            def area(lado: int) -> int:
                return lado * lado

            def dobro(lado: int) -> int:
                return 2 * area(lado)

            print(area(3), dobro(3))

            def area(lado: int, altura: int = 2) -> int:
                return lado * altura

            print(area(3), area(3, 4), dobro(3))
            for i in range(2):
                print(area(i + 1), dobro(i))

                def area(lado: int, altura: int = 5) -> int:
                    return lado + altura

            print(area(1), dobro(1))
        "},
    );
}

/// A call of a name with two `def`s runs each argument once, binds it by
/// the parameters of the function that runs, and keeps the frames of the
/// traceback.
#[test]
fn a_call_of_a_name_with_two_defs_binds_by_the_def_that_ran() {
    same_as_cpython(
        "a_call_of_a_name_with_two_defs_binds_by_the_def_that_ran",
        indoc! {"
            def mostra(n: int) -> int:
                print('arg', n)
                return n

            def divide(a: int, b: int = 1) -> int:
                return a // b

            def usa(x: int) -> int:
                return divide(mostra(x), b=mostra(2))

            print(usa(7))

            def divide(a: int, b: int = 0) -> int:
                return b // a

            print(usa(3))
            print(usa(0))
        "},
    );
}

/// A `def` in each branch of an `if` gives the name the function of the
/// branch that ran.
#[test]
fn a_call_of_a_name_with_defs_in_branches_calls_the_one_that_ran() {
    same_as_cpython(
        "a_call_of_a_name_with_defs_in_branches_calls_the_one_that_ran",
        indoc! {"
            def antes() -> int:
                return f(1)

            def f(x: int) -> int:
                return x

            def f(x: int) -> int:
                return x + 1

            print(antes())

            def sim(n: int) -> bool:
                return n > 0

            if sim(0):
                def g(x: int) -> int:
                    return x
            else:
                def g(x: int) -> int:
                    return -x
            print(g(1))
        "},
    );
}

/// A call before any `def` of a name with two raises `NameError`.
#[test]
fn a_call_before_the_defs_of_a_name_raises_name_error() {
    same_as_cpython(
        "a_call_before_the_defs_of_a_name_raises_name_error",
        indoc! {"
            def antes() -> int:
                return f(1)

            def g() -> int:
                return 0

            print(g())
            print(antes())

            def f(x: int) -> int:
                return x

            def f(x: int) -> int:
                return x + 1
        "},
    );
}

/// The `NameError` of a name near a name with two `def`s suggests it once
/// one of them ran.
#[test]
fn a_name_near_a_name_with_two_defs_suggests_it() {
    same_as_cpython(
        "a_name_near_a_name_with_two_defs_suggests_it",
        indoc! {"
            def area(x: int) -> int:
                return x

            def area(x: int) -> int:
                return 2 * x

            def errada() -> int:
                return aera(1)

            print(area(1))
            print(errada())

            def aera(x: int) -> int:
                return x
        "},
    );
}

#[test]
fn a_call_in_a_function_of_defs_with_two_types_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            def f() -> int:
                return 1

            def g() -> None:
                print(f())

            def f() -> str:
                return 'a'
        "}),
        "a call of a name whose functions give two types"
    );
}
