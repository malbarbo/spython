//! The `NameError` with the suggestion of CPython.

use indoc::{formatdoc, indoc};

use super::same_as_cpython;

#[test]
fn the_name_error_of_a_name_of_a_module_asks_about_an_import() {
    same_as_cpython(
        "the_name_error_of_a_name_of_a_module_asks_about_an_import",
        indoc! {"
            def h() -> str:
                return string

            print(h())
            string = 'a'
        "},
    );
    same_as_cpython(
        "the_name_error_of_a_call_of_a_name_of_a_module_asks_about_an_import",
        indoc! {"
            print(random())  # ty: ignore[unresolved-reference]

            def random() -> int:
                return 4
        "},
    );
}

#[test]
fn the_name_error_suggests_a_local_with_a_value() {
    same_as_cpython(
        "the_name_error_suggests_a_local_with_a_value",
        indoc! {"
            def f() -> int:
                totals = 1
                return total

            print(f())
            total = 2
        "},
    );
}

#[test]
fn the_name_error_suggests_no_local_without_a_value() {
    for (name, c) in [
        ("the_name_error_suggests_no_local_without_a_value", "False"),
        ("the_name_error_suggests_a_local_after_its_value", "True"),
    ] {
        same_as_cpython(
            name,
            &formatdoc! {"
                def f(c: bool) -> int:
                    if c:
                        totals = 1
                    return total

                print(f({c}))
                total = 2
                "},
        );
    }
}

/// CPython lists the locals in the order of its code, where the value of
/// an assignment comes before its target.
#[test]
fn the_name_error_suggests_the_first_local_in_the_order_of_the_code() {
    same_as_cpython(
        "the_name_error_suggests_the_first_local_in_the_order_of_the_code",
        indoc! {"
            def f() -> int:
                n = 0
                while n < 2:
                    if n == 1:
                        abe = abd  # ty: ignore[possibly-unresolved-reference]
                    abd = 1
                    n += 1
                return abc

            print(f())
            abc = 3
        "},
    );
}

/// CPython lists the globals in the order of their first binding, and
/// skips a global without a value.
#[test]
fn the_name_error_suggests_the_global_with_the_first_binding() {
    for (name, call) in [
        (
            "the_name_error_suggests_the_global_with_the_first_binding",
            "g(True)\n",
        ),
        (
            "the_name_error_suggests_the_other_global_with_the_first_binding",
            "g(False)\n",
        ),
        ("the_name_error_suggests_no_global_without_a_value", ""),
    ] {
        same_as_cpython(
            name,
            &formatdoc! {"
                def g(c: bool) -> None:
                    global kzy, kzw
                    if c:
                        kzy = 1
                        kzw = 2
                    else:
                        kzw = 2
                        kzy = 1
                    kzy = 3

                def f() -> int:
                    return kzx

                kzy: int
                kzw: int
                {call}print(f())
                kzx = 3
                "},
        );
    }
}

#[test]
fn the_name_error_suggests_a_function_after_its_def() {
    same_as_cpython(
        "the_name_error_suggests_a_function_after_its_def",
        indoc! {"
            def area() -> int:
                return 1

            print(areas)  # ty: ignore[unresolved-reference]
            areas = 2
        "},
    );
}

/// The globals of a module come before the globals of the program.
#[test]
fn the_name_error_suggests_a_global_of_the_module_first() {
    same_as_cpython(
        "the_name_error_suggests_a_global_of_the_module_first",
        indoc! {"
            __nams__ = 1
            print(__nam__)  # ty: ignore[unresolved-reference]
            __nam__ = 2
        "},
    );
}

#[test]
fn the_name_error_suggests_a_builtin() {
    same_as_cpython(
        "the_name_error_suggests_a_builtin",
        indoc! {"
            def f() -> int:
                return lenn

            print(f())
            lenn = 2
        "},
    );
}

/// CPython suggests an attribute of a `self` with a value.
#[test]
fn the_name_error_suggests_an_attribute_of_self() {
    same_as_cpython(
        "the_name_error_suggests_an_attribute_of_self",
        indoc! {"
            def f(self: str) -> int:
                return upper

            print(f('a'))
            upper = 2
        "},
    );
    same_as_cpython(
        "the_name_error_suggests_an_attribute_of_self_in_the_module",
        indoc! {"
            self = 1
            print(real)  # ty: ignore[unresolved-reference]
            real = 2
        "},
    );
}

#[test]
fn the_name_error_suggests_a_name_and_asks_about_an_import() {
    same_as_cpython(
        "the_name_error_suggests_a_name_and_asks_about_an_import",
        indoc! {"
            def f() -> int:
                strings = 1
                return string

            print(f())
            string = 2
        "},
    );
}

/// A program of `bound` globals with a value and `free` without, which
/// reads a name without a value in the module or in a function of `locals`
/// locals. CPython makes no suggestion with 750 candidates or more.
fn many_names(bound: usize, free: usize, locals: usize, in_function: bool) -> String {
    let mut source = String::from("abd = 0\n");
    for i in 1..bound {
        source.push_str(&format!("v{i} = {i}\n"));
    }
    if in_function {
        source.push_str("def f() -> int:\n");
        for i in 0..locals {
            source.push_str(&format!("    l{i} = {i}\n"));
        }
        source.push_str("    return abc\n\nprint(f())\n");
    } else {
        source.push_str("print(abc)  # ty: ignore[unresolved-reference]\n");
    }
    source.push_str("abc = 1\n");
    for i in 0..free {
        source.push_str(&format!("w{i} = {i}\n"));
    }
    source
}

#[test]
fn the_name_error_of_the_module_counts_its_globals_twice() {
    for bound in [286, 287] {
        same_as_cpython(
            &format!("the_name_error_of_the_module_with_{bound}_globals"),
            &many_names(bound, 10, 0, false),
        );
    }
}

#[test]
fn the_name_error_of_a_function_counts_its_locals_with_a_value() {
    // With f, the module has 9 globals of its own, `bound` + 1 of the
    // program, and 159 builtins.
    for (bound, locals) in [(500, 80), (500, 81)] {
        same_as_cpython(
            &format!("the_name_error_of_a_function_with_{bound}_globals_and_{locals}_locals"),
            &many_names(bound, 0, locals, true),
        );
    }
}

/// The local of the suggestion may be one that a `print` used before.
#[test]
fn the_name_error_without_a_suggestion_follows_a_print() {
    same_as_cpython(
        "the_name_error_without_a_suggestion_follows_a_print",
        indoc! {"
            def f(c: bool) -> int:
                print('olá')
                if c:
                    totals = 1
                return total

            print(f(False))
            total = 2
        "},
    );
}

/// A declaration without a value gives CPython no code, so it does not set
/// the place of the local.
#[test]
fn the_name_error_suggests_the_first_local_after_a_bare_declaration() {
    same_as_cpython(
        "the_name_error_suggests_the_first_local_after_a_bare_declaration",
        indoc! {"
            def f() -> int:
                ab: int
                ac = 1
                ab = 2
                return aa + ab + ac


            print(f())
            aa = 0
        "},
    );
}

/// CPython measures the distance between two names over their UTF-8
/// bytes, so `acao` is too far from `ação`, and `açao` is not.
#[test]
fn the_name_error_measures_a_name_outside_ascii_by_its_bytes() {
    for (name, typo) in [
        ("the_name_error_suggests_no_local_too_far_in_bytes", "acao"),
        ("the_name_error_suggests_a_local_outside_ascii", "açao"),
    ] {
        same_as_cpython(
            name,
            &formatdoc! {"
                def f() -> int:
                    ação = 1
                    return {typo}


                print(f())
                {typo} = 2
            "},
        );
    }
}

/// CPython looks at the locals before the builtins, and keeps the first
/// name at the smallest distance.
#[test]
fn the_name_error_suggests_a_local_over_a_builtin_at_the_same_distance() {
    same_as_cpython(
        "the_name_error_suggests_a_local_over_a_builtin_at_the_same_distance",
        indoc! {"
            def f() -> int:
                lex = 1
                return lem


            print(f())
            lem = 2
        "},
    );
}

/// A `bool` has the attributes of an `int`.
#[test]
fn the_name_error_suggests_an_attribute_of_a_bool_self() {
    same_as_cpython(
        "the_name_error_suggests_an_attribute_of_a_bool_self",
        indoc! {"
            def f(self: bool) -> int:
                return real


            print(f(True))
            real = 2
        "},
    );
}
