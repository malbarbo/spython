//! The functions of `math`.

use indoc::{formatdoc, indoc};

use super::{path, run, same_as_cpython, stops_at_the_int_limit, unsupported};

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

            print(math.comb(5, 2))
        "}),
        "this function of math"
    );
    assert_eq!(
        unsupported(indoc! {"
            import math

            print(math.hypot(1.0, 2.0, 3.0))
        "}),
        "this call of a function of math"
    );
    assert_eq!(
        unsupported(indoc! {"
            import math

            print(math.__name__)
        "}),
        "this name of math"
    );
}

#[test]
fn the_constants_of_math_follow_python() {
    same_as_cpython(
        "the_constants_of_math_follow_python",
        indoc! {"
            import math
            from math import pi, e as euler, inf, nan

            print(math.pi, math.e, math.tau, math.inf, -math.inf, math.nan)
            print(pi, euler, inf, nan, pi == math.pi, nan == nan, inf > 1e308)
        "},
    );
}

#[test]
fn a_constant_of_math_before_the_import_raises_name_error() {
    same_as_cpython(
        "a_constant_of_math_before_the_import_raises_name_error",
        indoc! {"
            def f() -> float:
                return pi

            print(f())
            from math import pi
        "},
    );
}

#[test]
fn sqrt_follows_python() {
    same_as_cpython(
        "sqrt_follows_python",
        indoc! {"
            import math

            inf = 1e308 * 10
            print(math.sqrt(2.0), math.sqrt(4), math.sqrt(True), math.sqrt(1073741823))
            print(math.sqrt(0.0), math.sqrt(-0.0), math.sqrt(5e-324), math.sqrt(1.7e308))
            print(math.sqrt(inf), math.sqrt(inf - inf))
        "},
    );
}

#[test]
fn fabs_and_copysign_follow_python() {
    same_as_cpython(
        "fabs_and_copysign_follow_python",
        indoc! {"
            import math
            from math import copysign

            inf = 1e308 * 10
            print(math.fabs(-3), math.fabs(2.5), math.fabs(-0.0), math.fabs(True), math.fabs(-inf))
            print(math.fabs(inf - inf), copysign(1, -0.0), copysign(-2.5, 3), copysign(True, -1))
            print(copysign(inf, -1.0), copysign(0.0, -inf), copysign(-1.0, inf - inf))
        "},
    );
}

#[test]
fn fmod_follows_python() {
    same_as_cpython(
        "fmod_follows_python",
        indoc! {"
            import math
            from math import fmod

            inf = 1e308 * 10
            print(fmod(7, 3), fmod(-7, 3), fmod(7.5, -2), fmod(-0.0, 2), fmod(True, 2))
            print(fmod(1e308, 3.3), fmod(5e-324, 1e-320), fmod(0.3, 0.1), math.fmod(3, inf))
            print(fmod(-3, -inf), fmod(inf - inf, 0), fmod(1, inf - inf))
        "},
    );
}

#[test]
fn fmod_raises_as_in_python() {
    for (name, call) in [
        ("fmod_by_0_raises_value_error", "math.fmod(1, 0)"),
        ("fmod_by_minus_0_raises_value_error", "math.fmod(2.5, -0.0)"),
        (
            "fmod_of_an_infinity_raises_value_error",
            "math.fmod(-1e308 * 10, 2)",
        ),
    ] {
        same_as_cpython(
            name,
            &formatdoc! {"
                import math

                print({call})
            "},
        );
    }
}

#[test]
fn gcd_follows_python() {
    same_as_cpython(
        "gcd_follows_python",
        indoc! {"
            import math
            from math import gcd

            print(gcd(), gcd(-4), gcd(0), gcd(12, 18), gcd(-12, 18), gcd(0, -7), gcd(0, 0))
            print(gcd(True, 4), gcd(1073741823, 1073741822), gcd(-1073741824, 6))
            print(gcd(12, 18, 8), math.gcd(-1073741824, 0, 3), gcd(-1073741824, -1073741824, 6))
            print(gcd(536870912, 1048576, 33554432))
        "},
    );
}

/// The `gcd` of the least `int` and 0 is one past the greatest `int`.
#[test]
fn gcd_that_does_not_fit_stops_the_program() {
    stops_at_the_int_limit(
        "gcd_that_does_not_fit_stops_the_program",
        indoc! {"
            import math

            print(math.gcd(-1073741824, 0))
        "},
        "",
        3,
        "    print(math.gcd(-1073741824, 0))\n          ~~~~~~~~^^^^^^^^^^^^^^^^\n",
        "1073741824",
    );
}

#[test]
fn factorial_follows_python() {
    same_as_cpython(
        "factorial_follows_python",
        indoc! {"
            import math
            from math import factorial

            print(factorial(0), factorial(1), factorial(5), factorial(True), math.factorial(12))
        "},
    );
}

#[test]
fn factorial_of_a_negative_int_raises_value_error() {
    same_as_cpython(
        "factorial_of_a_negative_int_raises_value_error",
        indoc! {"
            import math

            print(math.factorial(-1))
        "},
    );
}

/// `13!` is the least factorial that does not fit in an `int`.
#[test]
fn factorial_that_does_not_fit_stops_the_program() {
    for n in ["13", "1073741823"] {
        let name = format!("factorial_of_{n}_stops_the_program");
        let output = run(
            &name,
            &formatdoc! {"
                import math

                print(math.factorial({n}))
            "},
        );
        let carets = "^".repeat(n.len() + 2);
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            format!(
                "Traceback (most recent call last):\n  \
                 File \"{}\", line 3, in <module>\n    \
                 print(math.factorial({n}))\n          ~~~~~~~~~~~~~~{carets}\n\
                 spython: the result of factorial({n}) does not fit in an int, \
                 which goes from -1073741824 to 1073741823\n",
                path(&name, "py").display(),
            ),
            "stderr"
        );
        assert_eq!(output.status.code(), Some(1), "exit code");
    }
}

#[test]
fn sqrt_below_0_raises_value_error() {
    same_as_cpython(
        "sqrt_below_0_raises_value_error",
        indoc! {"
            from math import sqrt

            print(sqrt(1.0))
            print(sqrt(-1e-300))
        "},
    );
}

#[test]
fn isnan_isinf_and_isfinite_follow_python() {
    same_as_cpython(
        "isnan_isinf_and_isfinite_follow_python",
        indoc! {"
            import math
            from math import isnan, isinf, isfinite

            inf = 1e308 * 10
            print(math.isnan(inf - inf), math.isinf(inf - inf), math.isfinite(inf - inf))
            print(isnan(-inf), isinf(-inf), isfinite(-inf))
            print(isnan(1.7e308), isinf(1.7e308), isfinite(-5e-324))
            print(isnan(3), isinf(-1073741824), isfinite(True), math.isnan(-0.0))
        "},
    );
}

/// The `libm` crate can differ from the libm of glibc in the last bit, so
/// the program prints each result to 10 digits.
#[test]
fn the_functions_of_math_from_the_libm_follow_python() {
    same_as_cpython(
        "the_functions_of_math_from_the_libm_follow_python",
        indoc! {"
            import math
            from math import atan2, log

            def show(x: float) -> None:
                print(round(x, 10))

            x = 0.75
            show(math.exp(x))
            show(math.exp2(x))
            show(math.expm1(x))
            show(math.log(x))
            show(math.log2(x))
            show(math.log10(x))
            show(math.log1p(x))
            show(math.sin(x))
            show(math.cos(x))
            show(math.tan(x))
            show(math.asin(x))
            show(math.acos(x))
            show(math.atan(x))
            show(math.sinh(x))
            show(math.cosh(x))
            show(math.tanh(x))
            show(math.asinh(x))
            show(math.acosh(1 + x))
            show(math.atanh(x))
            show(math.cbrt(-x))
            show(math.erf(x))
            show(math.erfc(x))
            show(atan2(-1, -2.5))
            show(log(100, 10))
            show(log(True + 1, 4.0))
            inf = 1e308 * 10
            print(math.exp(-inf), math.atan(inf), math.tanh(-inf), math.asinh(-inf))
            print(math.log(inf), math.erfc(-inf), math.sin(inf - inf))
            print(atan2(0.0, -0.0), atan2(-0.0, -1.0), atan2(inf, -inf), atan2(inf - inf, 1))
        "},
    );
}

#[test]
fn the_functions_of_math_from_the_libm_raise_as_in_python() {
    for (name, call) in [
        ("exp_too_large_raises_overflow_error", "math.exp(1000)"),
        ("cosh_too_large_raises_overflow_error", "math.cosh(-1000.0)"),
        ("log_of_0_raises_value_error", "math.log(0)"),
        ("log10_below_0_raises_value_error", "math.log10(-2.0)"),
        ("log1p_of_minus_1_raises_value_error", "math.log1p(-1.0)"),
        (
            "sin_of_an_infinity_raises_value_error",
            "math.sin(1e308 * 10)",
        ),
        ("asin_above_1_raises_value_error", "math.asin(2)"),
        ("atanh_of_1_raises_value_error", "math.atanh(1.0)"),
        (
            "log_of_a_base_of_1_raises_zero_division_error",
            "math.log(2.0, 1)",
        ),
        (
            "log_of_a_base_of_0_raises_value_error",
            "math.log(2.0, 0.0)",
        ),
    ] {
        same_as_cpython(
            name,
            &formatdoc! {"
                import math

                print({call})
            "},
        );
    }
}

/// `math.gamma` and `math.lgamma` come from the code of CPython, which calls
/// `exp`, `pow` and `log` of the libm, so the program prints each result to
/// 10 digits.
#[test]
fn gamma_and_lgamma_follow_python() {
    same_as_cpython(
        "gamma_and_lgamma_follow_python",
        indoc! {"
            import math
            from math import gamma, lgamma

            def show(x: float) -> None:
                print(round(x, 10))

            print(gamma(1), gamma(5), gamma(23.0), gamma(True))
            show(gamma(0.5))
            show(gamma(-2.5))
            show(gamma(7.25))
            show(gamma(170.5) / 1e306)
            show(gamma(-150.5) * 1e262)
            show(lgamma(0.5))
            show(lgamma(-2.5))
            show(lgamma(30))
            show(lgamma(1e300) / 1e302)
            inf = 1e308 * 10
            print(lgamma(1), lgamma(2.0), gamma(-200.5), gamma(1e-30) / 1e30)
            print(gamma(inf), lgamma(inf), lgamma(-inf), gamma(inf - inf), lgamma(inf - inf))
        "},
    );
}

#[test]
fn gamma_and_lgamma_raise_as_in_python() {
    for (name, call) in [
        ("gamma_of_0_raises_value_error", "math.gamma(0.0)"),
        ("gamma_of_minus_0_raises_value_error", "math.gamma(-0.0)"),
        (
            "gamma_of_a_negative_integer_raises_value_error",
            "math.gamma(-3)",
        ),
        (
            "gamma_of_minus_inf_raises_value_error",
            "math.gamma(-1e308 * 10)",
        ),
        ("gamma_above_171_raises_overflow_error", "math.gamma(172)"),
        (
            "gamma_of_a_tiny_float_raises_overflow_error",
            "math.gamma(1e-310)",
        ),
        ("lgamma_of_0_raises_value_error", "math.lgamma(0)"),
        (
            "lgamma_of_a_negative_integer_raises_value_error",
            "math.lgamma(-7.0)",
        ),
        (
            "lgamma_too_large_raises_overflow_error",
            "math.lgamma(1e306)",
        ),
    ] {
        same_as_cpython(
            name,
            &formatdoc! {"
                import math

                print({call})
            "},
        );
    }
}

/// `math.hypot` comes from the code of CPython, which computes it with no
/// call to the libm, so the program prints each result in full.
#[test]
fn hypot_follows_python() {
    same_as_cpython(
        "hypot_follows_python",
        indoc! {"
            import math
            from math import hypot

            inf = 1e308 * 10
            print(hypot(3, 4), hypot(-5.0, 12), hypot(True, False), hypot(0.0, -0.0))
            print(hypot(1.5, 2.25), hypot(0.1, 0.2), hypot(1e308, 1e308), hypot(1e308, 1e308 * 2))
            print(hypot(5e-324, 5e-324), hypot(3e-310, 4e-310), hypot(1e-200, 1e200))
            print(math.hypot(inf - inf, -inf), math.hypot(inf - inf, 1), math.hypot(2, -inf))
        "},
    );
}
