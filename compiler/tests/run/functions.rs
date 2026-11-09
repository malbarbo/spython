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
    assert_eq!(
        unsupported(indoc! {"
            def f(n: int = 1) -> int:
                return n
        "}),
        "a parameter with a default"
    );
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
        "a function as a value"
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
        "a call with keyword arguments"
    );
}

#[test]
fn each_call_outside_what_the_compiler_covers_has_its_own_message() {
    assert_eq!(
        unsupported(indoc! {"
            print('a'.upper())
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
            print(chr(65))
        "}),
        "a call of a function that the module does not define"
    );
}
