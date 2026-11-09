//! The builtin functions of numbers.

use indoc::indoc;

use super::{path, run, same_as_cpython, stops_at_the_int_limit, unsupported};

#[test]
fn abs_follows_python() {
    same_as_cpython(
        "abs_follows_python",
        indoc! {"
            def magnitude(x: float) -> float:
                return abs(x)

            inf = 1e308 * 10
            n = -1073741823
            print(abs(-3), abs(3), abs(0), abs(n), abs(True), abs(False))
            print(abs(-2.5), magnitude(-0.0), magnitude(inf), magnitude(-inf), magnitude(inf - inf))
            print(abs(-3) + 1, abs(-1.5) * 2)
        "},
    );
}

#[test]
fn abs_of_the_least_int_stops_the_program() {
    stops_at_the_int_limit(
        "abs_of_the_least_int_stops_the_program",
        indoc! {"
            n = -1073741823 - 1
            print(abs(n))
        "},
        "",
        2,
        "    print(abs(n))\n          ~~~^^^\n",
        "1073741824",
    );
}

#[test]
fn min_and_max_follow_python() {
    same_as_cpython(
        "min_and_max_follow_python",
        indoc! {"
            def seen(x: int) -> int:
                print('seen', x)
                return x

            inf = 1e308 * 10
            nan = inf - inf
            print(min(3, 1, 2), max(3, 1, 2), min(-1073741824, 1073741823), max(5, 5))
            print(min(2.5, 1.5), max(2.5, 1.5), min(0.0, -0.0), min(-0.0, 0.0), max(0.0, -0.0))
            print(max(-0.0, 0.0), max(nan, 1.0), max(1.0, nan), min(nan, 1.0), min(1.0, nan, 0.5))
            print(min(-inf, inf), max(-inf, inf), max(1.5, 2.5, 0.5, 2.5))
            print(min('b', 'a', 'c'), max('b', 'a', 'c'), max('', 'a'), min('ab', 'a'))
            print(min(True, False), max(True, False), max(False, False))
            print(max(seen(1), seen(3), seen(2)) + 1)
        "},
    );
}

#[test]
fn min_and_max_outside_what_the_compiler_covers_have_their_own_message() {
    assert_eq!(
        unsupported(indoc! {"
            print(max('abc'))
        "}),
        "min or max of an iterable"
    );
    assert_eq!(
        unsupported(indoc! {"
            print(min(1, True))
        "}),
        "min or max of values of two types"
    );
    assert_eq!(
        unsupported(indoc! {"
            print(max(1, 2.5))
        "}),
        "min or max of values of two types"
    );
}

#[test]
fn bool_gives_the_truth_of_a_value() {
    same_as_cpython(
        "bool_gives_the_truth_of_a_value",
        indoc! {"
            def truth(x: float) -> bool:
                return bool(x)

            inf = 1e308 * 10
            print(bool(), bool(0), bool(-3), bool(True), bool(False), bool(''), bool('a'))
            print(truth(0.0), truth(-0.0), truth(5e-324), truth(inf - inf), bool(1 and 'a'))
            print(bool(not 0), bool(0 or ''), bool(2) + 1)
        "},
    );
}

#[test]
fn float_of_a_number_follows_python() {
    same_as_cpython(
        "float_of_a_number_follows_python",
        indoc! {"
            def same(x: float) -> float:
                return float(x)

            n = -1073741824
            print(float(), float(0), float(n), float(1073741823), float(True), float(False))
            print(same(-0.0), same(2.5), same(1e308 * 10), float(3) / 2)
        "},
    );
}

#[test]
fn float_of_a_str_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            print(float('1.5'))
        "}),
        "float of this type"
    );
}

#[test]
fn pow_of_two_values_is_the_power_operator() {
    same_as_cpython(
        "pow_of_two_values_is_the_power_operator",
        indoc! {"
            def power(x: float, n: int) -> float:
                return pow(x, n)

            print(pow(2.0, 10), pow(2, 0.5), pow(True, 2.0), power(-2.0, 3), power(1.5, -2))
            print(power(0.0, -1))
        "},
    );
}

#[test]
fn pow_outside_what_the_compiler_covers_has_its_own_message() {
    assert_eq!(
        unsupported(indoc! {"
            print(pow(3, 4, 5))
        "}),
        "this call of pow"
    );
}

#[test]
fn int_of_a_number_follows_python() {
    same_as_cpython(
        "int_of_a_number_follows_python",
        indoc! {"
            def whole(x: float) -> int:
                return int(x)

            print(int(), int(7), int(-1073741824), int(True), int(False))
            print(whole(2.9), whole(-2.9), whole(-0.0), whole(-0.5), whole(5e-324))
            print(whole(-1073741824.9), whole(1073741823.9), whole(1e9) + 1)
        "},
    );
}

#[test]
fn int_of_nan_raises_value_error() {
    same_as_cpython(
        "int_of_nan_raises_value_error",
        indoc! {"
            inf = 1e308 * 10
            print(int(1.5))
            print(int(inf - inf))
        "},
    );
}

#[test]
fn int_of_an_infinity_raises_overflow_error() {
    same_as_cpython(
        "int_of_an_infinity_raises_overflow_error",
        indoc! {"
            def whole(x: float) -> int:
                return int(x)

            print(whole(2.5))
            print(whole(-1e308 * 10))
        "},
    );
}

#[test]
fn int_of_a_float_outside_31_bits_raises_overflow_error() {
    for (name, value) in [
        (
            "int_of_a_float_above_31_bits_raises_overflow_error",
            "1073741824.0",
        ),
        (
            "int_of_a_float_below_31_bits_raises_overflow_error",
            "-1073741825.0",
        ),
        ("int_of_a_huge_float_raises_overflow_error", "1e300"),
    ] {
        let output = run(name, &format!("print(int({value}))\n"));
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            format!(
                "Traceback (most recent call last):\n  \
                 File \"{}\", line 1, in <module>\n    \
                 print(int({value}))\n          {}\n\
                 OverflowError: cannot convert float to an int of spython, \
                 which goes from -1073741824 to 1073741823\n",
                path(name, "py").display(),
                "~~~^".to_owned() + &"^".repeat(value.len() + 1),
            ),
            "stderr"
        );
        assert_eq!(output.status.code(), Some(1), "exit code");
    }
}

#[test]
fn round_of_a_number_follows_python() {
    same_as_cpython(
        "round_of_a_number_follows_python",
        indoc! {"
            def nearest(x: float) -> int:
                return round(x)

            print(round(2.5), round(3.5), round(-2.5), round(0.5), round(-0.5), round(1.5, None))
            print(nearest(2.4999999999999996), nearest(0.49999999999999994), nearest(-0.0))
            print(nearest(-1073741824.5), nearest(1073741823.4), nearest(4503599627370497.0 - 4503599627370496.0))
            print(round(7), round(-1073741824), round(True), round(False, None), nearest(1e9) + 1)
        "},
    );
}

#[test]
fn round_of_an_int_to_a_number_of_digits_follows_python() {
    same_as_cpython(
        "round_of_an_int_to_a_number_of_digits_follows_python",
        indoc! {"
            def seen(x: int) -> int:
                print('seen', x)
                return x

            print(round(1234, 2), round(1234, 0), round(1234, -1), round(1234, -2), round(-1234, -2))
            print(round(15, -1), round(25, -1), round(-15, -1), round(-25, -1), round(35, -1), round(5, -1))
            print(round(1073741823, -9), round(-1073741824, -9), round(-1073741824, -10), round(999, -20))
            print(round(500000000, -9), round(1073741823, -2), round(-500000000, -9))
            print(round(True, 2), round(True, -1), round(7, True), round(7, False), round(1073741823, 1073741823))
            print(round(seen(149), seen(-2)))
        "},
    );
}

#[test]
fn round_of_an_int_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "round_of_an_int_outside_31_bits_stops_the_program",
        indoc! {"
            print(round(1073741823, -8))
        "},
        "",
        1,
        "    print(round(1073741823, -8))\n          ~~~~~^^^^^^^^^^^^^^^^\n",
        "1100000000",
    );
}

#[test]
fn round_of_nan_raises_value_error() {
    same_as_cpython(
        "round_of_nan_raises_value_error",
        indoc! {"
            inf = 1e308 * 10
            print(round(1.5))
            print(round(inf - inf))
        "},
    );
}

#[test]
fn round_of_an_infinity_raises_overflow_error() {
    same_as_cpython(
        "round_of_an_infinity_raises_overflow_error",
        indoc! {"
            inf = 1e308 * 10
            print(round(-2.5))
            print(round(-inf, None))
        "},
    );
}

/// `1073741823.5` rounds to `1073741824`, one past the greatest `int`.
#[test]
fn round_of_a_float_outside_31_bits_raises_overflow_error() {
    let name = "round_of_a_float_outside_31_bits_raises_overflow_error";
    let output = run(name, "print(round(1073741823.5))\n");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "Traceback (most recent call last):\n  \
             File \"{}\", line 1, in <module>\n    \
             print(round(1073741823.5))\n          ~~~~~^^^^^^^^^^^^^^\n\
             OverflowError: cannot convert float to an int of spython, \
             which goes from -1073741824 to 1073741823\n",
            path(name, "py").display(),
        ),
        "stderr"
    );
    assert_eq!(output.status.code(), Some(1), "exit code");
}

#[test]
fn round_outside_what_the_compiler_covers_has_its_own_message() {
    assert_eq!(
        unsupported(indoc! {"
            print(round(2.5, 1))
        "}),
        "round of a float to a number of digits"
    );
    assert_eq!(
        unsupported(indoc! {"
            print(round(2.5, ndigits=1))
        "}),
        "this call of round"
    );
}
