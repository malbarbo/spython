//! Functions, their calls, and the globals that they read and assign.

use super::{same_as_cpython, unsupported};

#[test]
fn a_function_takes_its_arguments_and_returns_its_result() {
    same_as_cpython(
        "a_function_takes_its_arguments_and_returns_its_result",
        "def soma(a: int, b: int) -> int:\n    total = a + b\n    return total\n\
         \n\
         def maior(a: int, b: int) -> int:\n    if a > b:\n        return a\n    else:\n        return b\n\
         \n\
         def eco(s: str, vezes: int) -> None:\n    while vezes > 0:\n        print(s)\n        vezes -= 1\n\
         \n\
         def primeiro(n: int) -> int:\n    i = 2\n    while True:\n        if n % i == 0:\n            return i\n        i += 1\n\
         \n\
         def nada(b: bool) -> None:\n    if b:\n        return\n    print('nada')\n\
         \n\
         def par(n: int) -> bool:\n    return n % 2 == 0\n\
         \n\
         def texto(b: bool) -> str:\n    if b:\n        return 'sim'\n    return 'não'\n\
         \n\
         print(soma(2, 3), maior(soma(1, 1), 7), maior(9, 4))\n\
         eco('oi', 2)\n\
         print(primeiro(91), par(4), texto(par(3)))\n\
         nada(True)\n\
         nada(False)\n",
    );
}

#[test]
fn a_function_assigns_a_global_that_it_declares() {
    same_as_cpython(
        "a_function_assigns_a_global_that_it_declares",
        "count = 0\nname = 'a'\n\n\ndef bump(n: int) -> int:\n    global count, name\n    count += n\n    if count > 2:\n        name = name + 'b'\n    return count\n\n\ndef local() -> None:\n    count = 10\n    print(count)\n\n\nprint(bump(1), bump(2), count, name)\nlocal()\nprint(count)\n",
    );
}

#[test]
fn a_global_that_only_a_function_assigns_has_a_value_after_the_call() {
    same_as_cpython(
        "a_global_that_only_a_function_assigns_has_a_value_after_the_call",
        "name: str\n\n\ndef init() -> None:\n    global name\n    name = 'ana'\n\n\ndef show() -> None:\n    print(name)\n\n\ninit()\nshow()\nprint(name)  # ty: ignore[possibly-unresolved-reference]\n",
    );
}

#[test]
fn a_global_read_before_its_assignment_raises_name_error() {
    same_as_cpython(
        "a_global_read_before_its_assignment_raises_name_error",
        "def bump() -> None:\n    global total\n    total += 1\n\n\nbump()\ntotal = 0\n",
    );
}

#[test]
fn a_global_with_values_of_two_types_is_unsupported() {
    assert_eq!(
        unsupported(
            "x = 1\n\n\ndef f() -> None:\n    global x\n    x = 'a'  # ty: ignore[invalid-assignment]\n"
        ),
        "a variable with values of two types"
    );
}

#[test]
fn a_function_reads_the_globals_and_calls_a_function_defined_after_it() {
    same_as_cpython(
        "a_function_reads_the_globals_and_calls_a_function_defined_after_it",
        "base = 10\n\
         \n\
         def f(n: int) -> int:\n    return g(n) + base\n\
         \n\
         def g(n: int) -> int:\n    return n * fator\n\
         \n\
         fator = 3\n\
         print(f(2))\n\
         base = 100\n\
         print(f(2))\n",
    );
}

#[test]
fn an_exception_in_a_function_has_the_frames_of_the_calls() {
    same_as_cpython(
        "an_exception_in_a_function_has_the_frames_of_the_calls",
        "def k(n: int) -> int:\n    return 1 // n\n\
         \n\
         def g(n: int) -> int:\n    return k(n) + 1\n\
         \n\
         print(g(1))\n\
         print(k(2) + g(0))\n",
    );
}

#[test]
fn a_read_of_a_local_before_its_assignment_raises_unbound_local_error() {
    same_as_cpython(
        "a_read_of_a_local_before_its_assignment_raises_unbound_local_error",
        "def f(a: int) -> int:\n    if a > 0:\n        x = 1\n    return x + 7 // a  # ty: ignore[possibly-unresolved-reference]\n\
         \n\
         def g(n: int) -> int:\n    return f(n) + 1\n\
         \n\
         print(g(1))\n\
         print(g(0))\n",
    );
}

#[test]
fn a_read_of_a_global_before_its_assignment_raises_name_error() {
    same_as_cpython(
        "a_read_of_a_global_before_its_assignment_raises_name_error",
        "def h() -> int:\n    return y\n\
         \n\
         print(h())\n\
         y = 3\n",
    );
}

#[test]
fn a_call_before_the_def_raises_name_error() {
    same_as_cpython(
        "a_call_before_the_def_raises_name_error",
        "def g() -> int:\n    return f() + 1\n\
         \n\
         print('antes')\n\
         print(g())\n\
         \n\
         def f() -> int:\n    return 1\n",
    );
}

#[test]
fn a_call_before_the_def_in_the_module_raises_name_error() {
    same_as_cpython(
        "a_call_before_the_def_in_the_module_raises_name_error",
        "print(f())  # ty: ignore[unresolved-reference]\n\
         \n\
         def f() -> int:\n    return 1\n",
    );
}

#[test]
fn a_function_that_may_end_without_a_return_is_unsupported() {
    assert_eq!(
        unsupported(
            "def f(n: int) -> int:  # ty: ignore[invalid-return-type]\n    if n > 0:\n        return 1\n"
        ),
        "a function that may end without a return"
    );
    assert_eq!(
        unsupported(
            "def f(n: int) -> int:  # ty: ignore[invalid-return-type]\n    while True:\n        if n > 0:\n            break\n        return 1\n"
        ),
        "a function that may end without a return"
    );
    assert_eq!(
        unsupported(
            "def f(n: int) -> int:  # ty: ignore[invalid-return-type]\n    while True:\n        while n > 0:\n            n = n - 1\n        else:\n            break\n"
        ),
        "a function that may end without a return"
    );
}

#[test]
fn a_def_outside_what_the_compiler_covers_is_unsupported() {
    assert_eq!(
        unsupported("def f(n: int = 1) -> int:\n    return n\n"),
        "a parameter with a default"
    );
    assert_eq!(
        unsupported("def f(n: int) -> int:\n    return n\n\nprint(f)\n"),
        "a function as a value"
    );
    assert_eq!(
        unsupported("def f() -> None:\n    def g() -> None:\n        pass\n"),
        "a def in a function"
    );
    assert_eq!(
        unsupported(
            "def f() -> int:\n    return 1\n\nprint(f(n=1))  # ty: ignore[unknown-argument]\n"
        ),
        "a call with keyword arguments"
    );
}
