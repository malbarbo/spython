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
fn the_bit_operators_of_int_follow_python() {
    same_as_cpython(
        "the_bit_operators_of_int_follow_python",
        indoc! {"
            x = 12
            y = -5
            print(x & 10, x | 3, x ^ 5, y & 7, y | 2, y ^ -1, x & y)
            print(1073741823 | -1073741824, -1073741824 & -1, 1073741823 ^ -1)
            print(True & False, True | False, True ^ True, True & 3, 6 | False)
            z = 7
            z &= 5
            z |= 8
            z ^= 1
            b = True
            b &= False
            print(z, b)
        "},
    );
}

#[test]
fn the_shifts_and_the_inversion_of_int_follow_python() {
    same_as_cpython(
        "the_shifts_and_the_inversion_of_int_follow_python",
        indoc! {"
            def shift(n: int, k: int) -> None:
                print(n >> k, ~n)


            print(1 << 4, 256 >> 2, -7 >> 1, ~5, ~-1, ~0, ~True, ~False)
            print(~1073741823, ~-1073741824, -1 << 30, 1 << 29, -1073741824 >> 1)
            print(5 << 0, -5 << 3, 0 << 1000, -1 << 30, (-1 << 29) >> 1)
            shift(5, 0)
            shift(-5, 3)
            shift(0, 1000)
            shift(-1, 1000)
            shift(7, 31)
            shift(-7, 31)
            print(True << True, 6 >> True)
            n = 3
            n <<= 4
            n >>= 2
            print(n)
        "},
    );
    for (i, source) in ["print(5 << -1)", "print(0 >> -2)"].into_iter().enumerate() {
        same_as_cpython(&format!("a_negative_count_of_a_shift_raises_{i}"), source);
    }
}

#[test]
fn a_shift_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "a_shift_outside_31_bits_stops_the_program",
        indoc! {"
            print(1 << 29)
            print(1 << 30)
        "},
        "536870912\n",
        2,
        "    print(1 << 30)\n          ~~^^~~~\n",
        "of <<",
    );
    stops_at_the_int_limit(
        "a_shift_by_a_large_count_stops_the_program",
        indoc! {"
            k = len('a' * 100)
            print(1 << k)
        "},
        "",
        2,
        "    print(1 << k)\n          ~~^^~~\n",
        "of <<",
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

#[test]
fn a_power_of_two_ints_has_the_type_that_its_exponent_gives() {
    same_as_cpython(
        "a_power_of_two_ints_has_the_type_that_its_exponent_gives",
        indoc! {"
            def power(b: int, e: int) -> None:
                print(b ** e, pow(b, e))


            n = 3
            print(2 ** 10, (-3) ** 3, 0 ** 0, 7 ** 1, (-1024) ** 3, 2 ** True, 5 ** False, n ** 2)
            print(2 ** -1, (-2) ** -3, 10 ** -2, (-1) ** -1)
            power(2, 10)
            power(-2, 29)
            power(-64, 5)
            power(3, 0)
            power(2, -2)
            power(-1, 1000001)
            power(0, 5)
            power(1, 1073741823)
            power(5, -1)
            k = 2
            k **= 3
            print(k)
            power(0, -1)
        "},
    );
}

#[test]
fn pow_with_a_modulus_follows_python() {
    same_as_cpython(
        "pow_with_a_modulus_follows_python",
        indoc! {"
            def show(b: int, e: int, m: int) -> None:
                print(pow(b, e, m))


            print(pow(2, 10, 7), pow(-2, 3, 5), pow(2, 3, -5), pow(3, 0, -5), pow(5, 0, 1), pow(2, 3, None))
            show(3, -1, 7)
            show(-3, -5, 11)
            show(3, -1, -7)
            show(0, -1, 1)
            show(1073741822, 1073741823, 1073741823)
            show(-1073741824, -1073741824, 1073741823)
            show(1073741823, 1073741823, -1073741824)
            show(3, -1073741824, -1073741824)
        "},
    );
    for (i, source) in ["print(pow(2, 3, 0))", "print(pow(2, -1, 4))"]
        .into_iter()
        .enumerate()
    {
        same_as_cpython(&format!("pow_with_a_bad_modulus_raises_{i}"), source);
    }
}

#[test]
fn a_power_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "a_power_outside_31_bits_stops_the_program",
        indoc! {"
            print(2 ** 29)
            print(2 ** 30)
        "},
        "536870912\n",
        2,
        "    print(2 ** 30)\n          ~~^^~~~\n",
        "of **",
    );
    stops_at_the_int_limit(
        "a_power_with_a_variable_exponent_outside_31_bits_stops_the_program",
        indoc! {"
            e = len('a' * 40)
            print((-2) ** e)
        "},
        "",
        2,
        "    print((-2) ** e)\n          ~~~~~^^~~\n",
        "of **",
    );
}

#[test]
fn int_of_a_str_reads_it_as_cpython_does() {
    same_as_cpython(
        "int_of_a_str_reads_it_as_cpython_does",
        indoc! {"
            def read(s: str, base: int) -> None:
                print(repr(s), base, int(s, base))


            print(int('42'), int('  -17  '), int('+0'), int('1_000'), int('٣_4'), int('\\u3000 12\\u2003\\x85'))
            print(int('-1073741824'), int('1073741823'), int('00'), int('0' * 5000 + '7'), int('ff', base=16))
            read('0x1f', 16)
            read('0X1F', 0)
            read('0o17', 0)
            read('0b101', 0)
            read('0b_1', 0)
            read('0x_f', 16)
            read('z', 36)
            read('Z', 36)
            read('0_0', 0)
            read('000', 0)
            read('11', 2)
            read('0x', 36)
            read('1e3', 16)
            read('٠١٢', 3)
            read(' -0b1_1 ', 2)
            read('7', True + 7)
        "},
    );
    for (i, source) in [
        "int('')",
        "int(' ')",
        "int('1_')",
        "int('_1')",
        "int('1__0')",
        "int('-')",
        "int('0x1f')",
        "int('12abc')",
        "int('012', 0)",
        "int('1' * 4301)",
        "int('1' * 4301 + 'x')",
        "int('1' * 4301 + '  ')",
        "int('0' + '1' * 4301, 0)",
        "int('0' + '1' * 11, 0)",
        "int('5', 1)",
        "int('5', 37)",
        "int('5', -1)",
        "int('é')",
        "int('1\\x1c')",
        "int('1\\x00')",
        "int('x' * 300)",
        "int('0x', 16)",
        "int('0b2', 0)",
        "int('1 2')",
        "int('٣٤x', 0)",
    ]
    .into_iter()
    .enumerate()
    {
        same_as_cpython(
            &format!("int_of_an_invalid_str_raises_valueerror_{i}"),
            &format!("print({source})\n"),
        );
    }
}

#[test]
fn int_of_a_str_outside_31_bits_stops_the_program() {
    for (i, source) in [
        "int('1073741824')",
        "int('-1073741825')",
        "int('9' * 40)",
        "int('7fffffff', 16)",
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("int_of_a_str_outside_31_bits_stops_the_program_{i}");
        let output = super::run(&name, &format!("print({source})\n"));
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).lines().last(),
            Some(
                "spython: the result of int does not fit in an int, which goes from \
                 -1073741824 to 1073741823"
            ),
            "{source}"
        );
        assert_eq!(output.status.code(), Some(1));
    }
}
