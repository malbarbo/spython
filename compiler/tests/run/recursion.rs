//! The recursion limit and the traceback of a recursion.

use super::same_as_cpython;

#[test]
fn a_recursive_function_runs_up_to_the_limit_of_cpython() {
    same_as_cpython(
        "a_recursive_function_runs_up_to_the_limit_of_cpython",
        "def count(n: int) -> int:\n    if n == 0:\n        return 0\n    return 1 + count(n - 1)\n\
         \n\
         print(count(998))\n\
         print(count(999))\n",
    );
}

#[test]
fn a_recursion_without_end_raises_recursion_error() {
    same_as_cpython(
        "a_recursion_without_end_raises_recursion_error",
        "def f(n: int) -> int:\n    return f(n + 1)\n\nprint(f(0))\n",
    );
}

#[test]
fn the_traceback_writes_how_many_frames_on_the_same_line_it_leaves_out() {
    for (name, n) in [
        ("the_traceback_leaves_out_no_frame", 2),
        ("the_traceback_leaves_out_one_frame", 4),
        ("the_traceback_leaves_out_two_frames", 5),
    ] {
        same_as_cpython(
            name,
            &format!(
                "def f(n: int) -> int:\n    if n == 0:\n        return 1 // n  # ty: ignore[division-by-zero]\n    return f(n - 1)\n\
                 \n\
                 print(f({n}))\n"
            ),
        );
    }
}

#[test]
fn a_mutual_recursion_has_every_frame_in_the_traceback() {
    same_as_cpython(
        "a_mutual_recursion_has_every_frame_in_the_traceback",
        "def a(n: int) -> int:\n    return b(n)\n\
         \n\
         def b(n: int) -> int:\n    if n == 0:\n        return 1 // n  # ty: ignore[division-by-zero]\n    return a(n - 1)\n\
         \n\
         print(a(3))\n",
    );
}
