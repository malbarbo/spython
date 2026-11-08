//! The operators of `int`, its limit of 31 bits, and the division by 0.

use indoc::indoc;

use super::{same_as_cpython, stops_at_the_int_limit, unsupported};

#[test]
fn print_writes_an_int() {
    same_as_cpython(
        "print_writes_an_int",
        indoc! {"
            print(0, 7, -7, 10, 1073741823, -1073741824)
        "},
    );
}

#[test]
fn the_operators_of_int_follow_python() {
    same_as_cpython(
        "the_operators_of_int_follow_python",
        indoc! {"
            print(2 + 3 * 4, 7 - 10, -(3 - 5), +4, 2 * -3, 32768 * 32767)
        "},
    );
}

#[test]
fn a_bool_is_an_int_in_an_operator() {
    same_as_cpython(
        "a_bool_is_an_int_in_an_operator",
        indoc! {"
            print(True, False, True + True, -True, +False, True * 3)
        "},
    );
}

#[test]
fn the_comparisons_of_int_give_a_bool() {
    same_as_cpython(
        "the_comparisons_of_int_give_a_bool",
        indoc! {"
            print(1 < 2, 2 <= 1, 3 == 3, 3 != 3, 4 > 5, 5 >= 5, -1 < 0, False < True)
            print(1 < 2 < 3, 1 < 3 < 2, 3 > 2 > 1 > 0, 1 == 1 != 2)
        "},
    );
}

#[test]
fn a_chain_of_comparisons_stops_at_the_first_false() {
    same_as_cpython(
        "a_chain_of_comparisons_stops_at_the_first_false",
        indoc! {"
            print(2 < 1 < 1073741823 + 1)
        "},
    );
}

#[test]
fn an_int_expression_as_a_statement_prints_nothing() {
    same_as_cpython(
        "an_int_expression_as_a_statement_prints_nothing",
        indoc! {"
            1 + 2
            2 < 3
            True
            print(1)
        "},
    );
}

#[test]
fn a_sum_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "a_sum_outside_31_bits_stops_the_program",
        indoc! {"
            print('antes')
            print(1073741823 + 1)
            print('depois')
        "},
        "antes\n",
        2,
        "    print(1073741823 + 1)\n          ~~~~~~~~~~~^~~\n",
        "1073741824",
    );
}

#[test]
fn a_difference_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "a_difference_outside_31_bits_stops_the_program",
        indoc! {"
            print(-1073741824 - 1)
        "},
        "",
        1,
        "    print(-1073741824 - 1)\n          ~~~~~~~~~~~~^~~\n",
        "-1073741825",
    );
}

#[test]
fn a_product_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "a_product_outside_31_bits_stops_the_program",
        indoc! {"
            print(65536 * 65536)
        "},
        "",
        1,
        "    print(65536 * 65536)\n          ~~~~~~^~~~~~~\n",
        "4294967296",
    );
}

#[test]
fn the_negation_of_the_least_int_stops_the_program() {
    stops_at_the_int_limit(
        "the_negation_of_the_least_int_stops_the_program",
        indoc! {"
            print(-(-1073741823 - 1))
        "},
        "",
        1,
        "    print(-(-1073741823 - 1))\n          ^^^^^^^^^^^^^^^^^^\n",
        "1073741824",
    );
}

#[test]
fn an_int_literal_outside_31_bits_is_unsupported() {
    for source in [
        indoc! {"
        print(1073741824)
    "},
        indoc! {"
        print(-1073741825)
    "},
    ] {
        assert_eq!(unsupported(source), "an int that does not fit in 31 bits");
    }
}

#[test]
fn the_floor_division_and_the_modulo_follow_python() {
    same_as_cpython(
        "the_floor_division_and_the_modulo_follow_python",
        indoc! {"
            print(7 // 2, -7 // 2, 7 // -2, -7 // -2, 6 // 3, -6 // 3, 0 // -5)
            print(7 % 3, -7 % 3, 7 % -3, -7 % -3, 6 % 3, -6 % -3, 0 % 5)
            print(-1073741824 // 1, 1073741823 // -1, -1073741824 % -1, True // True)
        "},
    );
}

#[test]
fn the_floor_division_of_the_least_int_by_minus_one_stops_the_program() {
    stops_at_the_int_limit(
        "the_floor_division_of_the_least_int_by_minus_one_stops_the_program",
        indoc! {"
            print(-1073741824 // -1)
        "},
        "",
        1,
        "    print(-1073741824 // -1)\n          ~~~~~~~~~~~~^^~~~\n",
        "1073741824",
    );
}

// The checker rejects a divisor that it knows to be 0, unless a comment
// silences the error.
#[test]
fn the_floor_division_by_zero_raises_zero_division_error() {
    same_as_cpython(
        "the_floor_division_by_zero_raises_zero_division_error",
        indoc! {"
            print(1)
            print(7 // 0)  # ty: ignore[division-by-zero]
        "},
    );
}

#[test]
fn the_modulo_by_zero_raises_zero_division_error() {
    same_as_cpython(
        "the_modulo_by_zero_raises_zero_division_error",
        indoc! {"
            print(-7 % 0)  # ty: ignore[division-by-zero]
        "},
    );
}

#[test]
fn the_true_division_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
        print(7 / 2)
    "}),
        "this operator"
    );
}

#[test]
fn the_traceback_of_the_int_limit_shows_every_line_of_the_operator() {
    stops_at_the_int_limit(
        "the_traceback_of_the_int_limit_shows_every_line_of_the_operator",
        indoc! {"
            print('日本', (1073741823 +
                   1))
        "},
        "",
        1,
        "    print('日本', (1073741823 +\n                   ~~~~~~~~~~~^\n           1))\n           ~\n",
        "1073741824",
    );
}

#[test]
fn the_traceback_of_the_int_limit_has_no_carets_under_a_whole_line() {
    stops_at_the_int_limit(
        "the_traceback_of_the_int_limit_has_no_carets_under_a_whole_line",
        indoc! {"
            -(-1073741823 - 1)
        "},
        "",
        1,
        "    -(-1073741823 - 1)\n",
        "1073741824",
    );
}
