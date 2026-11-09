//! The functions of `math`.

use indoc::{formatdoc, indoc};

use super::{path, run, same_as_cpython, unsupported};

#[test]
fn floor_ceil_and_trunc_follow_python() {
    same_as_cpython(
        "floor_ceil_and_trunc_follow_python",
        indoc! {"
            import math
            import math as m
            from math import floor, ceil as c, trunc

            x = 0
            while x < 9:
                v = 0.0
                if x == 0:
                    v = 2.5
                elif x == 1:
                    v = -2.5
                elif x == 2:
                    v = 2.0
                elif x == 3:
                    v = -0.0
                elif x == 4:
                    v = 0.1
                elif x == 5:
                    v = -0.1
                elif x == 6:
                    v = 1e-300
                elif x == 7:
                    v = -1073741823.5
                else:
                    v = 1073741822.5
                print(math.floor(v), m.ceil(v), trunc(v), floor(v), c(v), math.trunc(v))
                x += 1
            print(math.floor(7), math.ceil(-7), math.trunc(1073741823), floor(True), c(False))
        "},
    );
}

#[test]
fn floor_ceil_and_trunc_of_nan_raise_value_error() {
    for function in ["floor", "ceil", "trunc"] {
        let name = format!("{function}_of_nan_raises_value_error");
        same_as_cpython(
            &name,
            &formatdoc! {"
                import math

                inf = 1e308 * 10
                print(math.{function}(inf - inf))
            "},
        );
    }
}

#[test]
fn floor_ceil_and_trunc_of_an_infinity_raise_overflow_error() {
    for function in ["floor", "ceil", "trunc"] {
        let name = format!("{function}_of_an_infinity_raises_overflow_error");
        same_as_cpython(
            &name,
            &formatdoc! {"
                import math

                inf = 1e308 * 10
                print(math.{function}(-inf))
            "},
        );
    }
}

/// `1073741823.5` goes up to `1073741824`, one past the greatest `int`.
#[test]
fn ceil_of_a_float_outside_31_bits_raises_overflow_error() {
    let name = "ceil_of_a_float_outside_31_bits_raises_overflow_error";
    let output = run(
        name,
        indoc! {"
            import math

            print(math.ceil(1073741823.5))
        "},
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "Traceback (most recent call last):\n  \
             File \"{}\", line 3, in <module>\n    \
             print(math.ceil(1073741823.5))\n          ~~~~~~~~~^^^^^^^^^^^^^^\n\
             OverflowError: cannot convert float to an int of spython, \
             which goes from -1073741824 to 1073741823\n",
            path(name, "py").display(),
        ),
        "stderr"
    );
    assert_eq!(output.status.code(), Some(1), "exit code");
}

#[test]
fn a_function_of_math_before_the_import_raises_name_error() {
    for (name, call, import) in [
        (
            "a_function_of_math_before_the_import_raises_name_error",
            "math.floor",
            "import math",
        ),
        (
            "a_function_from_math_before_the_import_raises_name_error",
            "floor",
            "from math import floor",
        ),
    ] {
        same_as_cpython(
            name,
            &formatdoc! {"
                def f() -> int:
                    return {call}(1.5)

                print(f())
                {import}
                print(f())
            "},
        );
    }
}

#[test]
fn math_outside_what_the_compiler_covers_has_its_own_message() {
    assert_eq!(
        unsupported(indoc! {"
            import math

            print(math.sqrt(2.0))
        "}),
        "this function of math"
    );
}
