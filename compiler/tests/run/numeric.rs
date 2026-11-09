//! The builtin functions of numbers.

use indoc::indoc;

use super::{same_as_cpython, stops_at_the_int_limit, unsupported};

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
