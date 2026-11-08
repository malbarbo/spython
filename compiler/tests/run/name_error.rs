//! The `NameError` with the suggestion of CPython.

use indoc::indoc;

use super::same_as_cpython;

#[test]
fn the_name_error_of_a_name_of_a_module_asks_about_an_import() {
    same_as_cpython(
        "the_name_error_of_a_name_of_a_module_asks_about_an_import",
        "def h() -> str:\n    return string\n\
         \n\
         print(h())\n\
         string = 'a'\n",
    );
    same_as_cpython(
        "the_name_error_of_a_call_of_a_name_of_a_module_asks_about_an_import",
        "print(random())  # ty: ignore[unresolved-reference]\n\
         \n\
         def random() -> int:\n    return 4\n",
    );
}

#[test]
fn the_name_error_suggests_a_local_with_a_value() {
    same_as_cpython(
        "the_name_error_suggests_a_local_with_a_value",
        "def f() -> int:\n    totals = 1\n    return total\n\
         \n\
         print(f())\n\
         total = 2\n",
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
            &format!(
                "def f(c: bool) -> int:\n    if c:\n        totals = 1\n    return total\n\
                 \n\
                 print(f({c}))\n\
                 total = 2\n"
            ),
        );
    }
}

/// CPython lists the locals in the order of its code, where the value of
/// an assignment comes before its target.
#[test]
fn the_name_error_suggests_the_first_local_in_the_order_of_the_code() {
    same_as_cpython(
        "the_name_error_suggests_the_first_local_in_the_order_of_the_code",
        "def f() -> int:\n    n = 0\n    while n < 2:\n        if n == 1:\n            \
         abe = abd  # ty: ignore[possibly-unresolved-reference]\n        abd = 1\n        \
         n += 1\n    return abc\n\
         \n\
         print(f())\n\
         abc = 3\n",
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
            &format!(
                "def g(c: bool) -> None:\n    global kzy, kzw\n    if c:\n        kzy = 1\n        \
                 kzw = 2\n    else:\n        kzw = 2\n        kzy = 1\n    kzy = 3\n\
                 \n\
                 def f() -> int:\n    return kzx\n\
                 \n\
                 kzy: int\n\
                 kzw: int\n\
                 {call}\
                 print(f())\n\
                 kzx = 3\n"
            ),
        );
    }
}

#[test]
fn the_name_error_suggests_a_function_after_its_def() {
    same_as_cpython(
        "the_name_error_suggests_a_function_after_its_def",
        "def area() -> int:\n    return 1\n\
         \n\
         print(areas)  # ty: ignore[unresolved-reference]\n\
         areas = 2\n",
    );
}

/// The globals of a module come before the globals of the program.
#[test]
fn the_name_error_suggests_a_global_of_the_module_first() {
    same_as_cpython(
        "the_name_error_suggests_a_global_of_the_module_first",
        "__nams__ = 1\n\
         print(__nam__)  # ty: ignore[unresolved-reference]\n\
         __nam__ = 2\n",
    );
}

#[test]
fn the_name_error_suggests_a_builtin() {
    same_as_cpython(
        "the_name_error_suggests_a_builtin",
        "def f() -> int:\n    return lenn\n\
         \n\
         print(f())\n\
         lenn = 2\n",
    );
}

/// CPython suggests an attribute of a `self` with a value.
#[test]
fn the_name_error_suggests_an_attribute_of_self() {
    same_as_cpython(
        "the_name_error_suggests_an_attribute_of_self",
        "def f(self: str) -> int:\n    return upper\n\
         \n\
         print(f('a'))\n\
         upper = 2\n",
    );
    same_as_cpython(
        "the_name_error_suggests_an_attribute_of_self_in_the_module",
        "self = 1\n\
         print(real)  # ty: ignore[unresolved-reference]\n\
         real = 2\n",
    );
}

#[test]
fn the_name_error_suggests_a_name_and_asks_about_an_import() {
    same_as_cpython(
        "the_name_error_suggests_a_name_and_asks_about_an_import",
        "def f() -> int:\n    strings = 1\n    return string\n\
         \n\
         print(f())\n\
         string = 2\n",
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
