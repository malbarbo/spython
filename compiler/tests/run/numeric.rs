//! The builtin functions of numbers.

use indoc::indoc;

use super::{same_as_cpython, stops_at_the_int_limit};

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
