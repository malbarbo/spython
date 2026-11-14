//! The `float` literals, the `float` in variables, parameters and results,
//! and the operators on a `float`.

use std::path::Path;

use indoc::indoc;
use wasmparser::{DataKind, Parser, Payload};

use super::{compile, same_as_cpython, unsupported_at};

#[test]
fn print_writes_the_repr_of_a_float() {
    same_as_cpython(
        "print_writes_the_repr_of_a_float",
        indoc! {"
            print(2.5, 0.0, 0.1, 100.0, 123456789.0, 0.30000000000000004)
            print(1e16, 1e15, 1e-05, 0.0001, 1e22, 1.5e300, 5e-324, 1e309)
            print('x =', 1.5, 2)
        "},
    );
}

#[test]
fn a_float_goes_through_variables_and_functions() {
    same_as_cpython(
        "a_float_goes_through_variables_and_functions",
        indoc! {"
            def same(x: float) -> float:
                y = x
                return y

            z = 2.5
            w: float = 0.1
            print(same(z), z, w)
        "},
    );
}

#[test]
fn the_truth_of_a_float_follows_python() {
    same_as_cpython(
        "the_truth_of_a_float_follows_python",
        indoc! {"
            inf = 1e308 * 10
            nan = inf - inf
            print(not 0.0, not -0.0, not 5e-324, not 1.5, not -inf, not nan)
            print(0.0 or 2.5, -0.0 or 0.0, 1.5 or 2.5, nan or 1.0)
            print(0.0 and 2.5, -0.0 and 1.0, 1.5 and 2.5, nan and -0.0)
            if nan:
                print('nan is true')
            if -0.0:
                print('-0.0 is true')
            else:
                print('-0.0 is false')
            x = 3.0
            while x:
                x = x - 1
                print(x)
        "},
    );
}

#[test]
fn comparisons_of_floats_follow_python() {
    same_as_cpython(
        "comparisons_of_floats_follow_python",
        indoc! {"
            inf = 1e308 * 10
            nan = inf - inf
            print(0.0 == -0.0, 0.0 < -0.0, 0.1 + 0.2 == 0.3, 0.1 + 0.2 > 0.3)
            print(nan == nan, nan != nan, nan < 1.0, nan >= 1.0, 1.0 > nan, nan <= nan)
            t = True
            print(1 == 1.0, 2 > 1.5, t > 0.5, 1073741823 < 1073741823.5, -inf < -1073741824)
            print(1 < 1.5 < 2, 1 < 2 < 1.5, 1 < nan < 2, 1.5 < 2 < 3, 0.0 == 0 == -0.0)
            print(inf > 1e308, inf == inf, -inf != inf, 5e-324 > 0)
        "},
    );
}

#[test]
fn add_sub_and_mul_on_floats() {
    same_as_cpython(
        "add_sub_and_mul_on_floats",
        indoc! {"
            def area(w: float, h: float) -> float:
                return w * h

            x = 0.1
            print(x + 0.2, 2.5 - x, area(1.5, x), 1e308 * 10, -1e308 * 10)
            big = 1e308 * 10
            print(big - big, 0.1 * 3 - 0.3)
            y = 1.5
            y += 1
            y -= 0.25
            y *= 2
            print(y)
        "},
    );
}

#[test]
fn an_int_or_a_bool_mixes_with_a_float() {
    same_as_cpython(
        "an_int_or_a_bool_mixes_with_a_float",
        indoc! {"
            n = 1073741823
            print(n * 1.5, 1.5 * n, -1073741824 + 0.5, 3 - 0.5)
            print(True + 1.5, 1.5 - False, True * 0.5)
        "},
    );
}

#[test]
fn the_sign_of_a_float_follows_python() {
    same_as_cpython(
        "the_sign_of_a_float_follows_python",
        indoc! {"
            x = 0.0
            print(-x, 0 - x, -x + 0.0, -x - 0.0, -x * 1, +x, -(-x))
            y = 2.5
            print(-y, +y, --y, -1.5, -0 * -1.5)
        "},
    );
}

#[test]
fn the_true_division_gives_a_float() {
    same_as_cpython(
        "the_true_division_gives_a_float",
        indoc! {"
            print(7 / 2, -7 / 2, 1 / 3, 2 / 1, 1073741823 / 7, -1073741824 / -1)
            print(0 / 5, 0 / -5, -0 / 5, True / 2, 3 / True)
            print(1.5 / 0.5, 1 / 3.0, 0.1 / 3, -0.0 / 1, 0.0 / -2.5, 1e308 / 1e-10)
            x = 5.0
            x /= 4
            print(x)
        "},
    );
}

#[test]
fn the_true_division_of_ints_by_zero_raises_zero_division_error() {
    same_as_cpython(
        "the_true_division_of_ints_by_zero_raises_zero_division_error",
        indoc! {"
            print(1 / 2)
            print(7 / 0)  # ty: ignore[division-by-zero]
        "},
    );
}

#[test]
fn the_true_division_of_bools_by_false_raises_zero_division_error() {
    same_as_cpython(
        "the_true_division_of_bools_by_false_raises_zero_division_error",
        indoc! {"
            def ratio(a: bool, b: bool) -> float:
                return a / b

            print(ratio(True, True))
            print(ratio(True, False))
        "},
    );
}

#[test]
fn the_true_division_by_a_float_zero_raises_zero_division_error() {
    same_as_cpython(
        "the_true_division_by_a_float_zero_raises_zero_division_error",
        indoc! {"
            def inverse(x: float) -> float:
                return 1 / x

            print(inverse(2.0))
            print(inverse(-0.0))
        "},
    );
}

#[test]
fn the_true_division_of_a_float_by_an_int_zero_raises_zero_division_error() {
    same_as_cpython(
        "the_true_division_of_a_float_by_an_int_zero_raises_zero_division_error",
        indoc! {"
            def scale(x: float, n: int) -> float:
                x /= n
                return x

            print(scale(2.5, 2))
            print(scale(2.5, 0))
        "},
    );
}

#[test]
fn floor_div_and_mod_on_floats_follow_cpython() {
    same_as_cpython(
        "floor_div_and_mod_on_floats_follow_cpython",
        indoc! {"
            inf = 1e308 * 10
            print(7.5 // 2, 7.5 % 2, -7.5 // 2, -7.5 % 2, 7.5 // -2, 7.5 % -2)
            print(7 // 2.5, 7 % -2.5, True // 0.5, 5 % -0.5, -5 // 0.5)
            print(0.0 % -2, -0.0 // 1, -0.0 % 1, 0.1 % 0.01, 1e16 % 3, 1e308 // 1e-308)
            print(1.0 % inf, -1.0 % inf, 1.0 // inf, -1.0 // inf, inf // 1.0, inf % 1.0)
            x = 10.0
            x //= 3
            y = 10.0
            y %= -3
            print(x, y)
        "},
    );
}

#[test]
fn the_floor_division_by_a_float_zero_raises_zero_division_error() {
    same_as_cpython(
        "the_floor_division_by_a_float_zero_raises_zero_division_error",
        indoc! {"
            def floor(x: float, y: float) -> float:
                return x // y

            print(floor(7.5, 2.0))
            print(floor(7.5, -0.0))
        "},
    );
}

#[test]
fn the_modulo_by_an_int_zero_raises_zero_division_error() {
    same_as_cpython(
        "the_modulo_by_an_int_zero_raises_zero_division_error",
        indoc! {"
            def rest(x: float, n: int) -> float:
                x %= n
                return x

            print(rest(7.5, 2))
            print(rest(7.5, 0))
        "},
    );
}

#[test]
fn powers_of_floats_follow_cpython() {
    same_as_cpython(
        "powers_of_floats_follow_cpython",
        indoc! {"
            def power(x: float, n: int) -> float:
                return x ** n

            inf = 1e308 * 10
            nan = inf - inf
            print(2.0 ** 10, 2 ** 0.5, 10 ** -0.5, 1.5 ** 2.5, 3.0 ** 34, 1e308 ** 0.5)
            print(2.0 ** -1074, 2.0 ** -1075, 1.0000001 ** 1e9, 0.5 ** -1022.5, 7.5 ** 1e-300)
            print(power(-2.0, 3), power(-2.0, 4), power(-0.0, 3), power(-0.0, 2), power(0.0, 0))
            print(power(nan, 0), power(nan, 1), power(-1.0, 1073741823), power(10.0, -400))
            print(power(inf, -1), power(-inf, 3), power(-inf, -3), power(-inf, 2), power(0.0, 5))
            print(2.0 ** inf, 0.5 ** inf, 2.0 ** -inf, 0.5 ** -inf, 1.0 ** inf, 1.0 ** nan)
            print(2.0 ** nan, 446.64 ** -95.901, 1.3e-164 ** 1.697913)
            x = -3.0
            print(x ** 2.0, x ** 3.0, x ** 0.0, x ** 1.0, True ** 2.0)
            x **= 3
            print(x)
        "},
    );
}

#[test]
fn a_power_outside_the_range_of_a_float_raises_overflow_error() {
    same_as_cpython(
        "a_power_outside_the_range_of_a_float_raises_overflow_error",
        indoc! {"
            def power(x: float, n: int) -> float:
                return x ** n

            print(power(10.0, 308), power(-10.0, -323))
            print(power(-10.0, 309))
        "},
    );
}

#[test]
fn zero_to_a_negative_power_raises_zero_division_error() {
    same_as_cpython(
        "zero_to_a_negative_power_raises_zero_division_error",
        indoc! {"
            def inverse(x: float) -> float:
                return x ** -1.0

            print(inverse(4.0))
            print(inverse(-0.0))
        "},
    );
}

/// A program without a `float` leaves out `f64_repr` and its data, which
/// are the only active segments.
#[test]
fn a_program_without_a_float_links_no_builtin() {
    let module = compile("print(1)\n", Path::new("no_float.py")).expect("the program to compile");
    for payload in Parser::new(0).parse_all(&module.bytes) {
        if let Payload::DataSection(s) = payload.expect("a valid module") {
            for data in s {
                let data = data.expect("a segment");
                assert!(matches!(data.kind, DataKind::Passive), "an active segment");
            }
        }
    }
}

#[test]
fn float_of_a_str_reads_it_as_cpython_does() {
    same_as_cpython(
        "float_of_a_str_reads_it_as_cpython_does",
        indoc! {"
            print(float('1.5'), float(' -2 '), float('.5'), float('5.'), float('1.e5'), float('1e1_0'))
            print(float('+nan'), float('-InFiNiTy'), float('iNf'), float('1e400'), float('1.5e-400'), float('-0'))
            print(float('1_0.2_5e-1_0'), float('١.٥'), float('\\u3000 7\\x85'), float('0' * 5000 + '1.25'))
            print(float('0.1'), float('9007199254740993'), float('2.4703282292062328e-324'), float('1' * 400))
        "},
    );
    for (i, source) in [
        "float('')",
        "float('é')",
        "float('x' * 300)",
        "float('1_')",
        "float('1__0')",
        "float('_1')",
        "float('inf_')",
        "float('.')",
        "float('1e')",
        "float('1e+')",
        "float('0x1p3')",
        "float('1\\x00')",
        "float('nan(1)')",
        "float('infinit')",
        "float('.e5')",
        "float('e5')",
        "float('++1')",
        "float('1,5')",
        "float('\\x1c1')",
        "float('1_.5')",
        "float('1._5')",
        "float('1e_5')",
    ]
    .into_iter()
    .enumerate()
    {
        same_as_cpython(
            &format!("float_of_an_invalid_str_raises_valueerror_{i}"),
            &format!("print({source})\n"),
        );
    }
}

#[test]
fn is_integer_of_a_float_follows_python() {
    same_as_cpython(
        "is_integer_of_a_float_follows_python",
        indoc! {"
            for x in [2.0, 2.5, -0.0, -3.0, 1e300, 1e-300, float('inf'), float('-inf'), float('nan')]:
                print(x, x.is_integer())
        "},
    );
}

#[test]
fn hex_and_conjugate_of_a_number_follow_python() {
    same_as_cpython(
        "hex_and_conjugate_of_a_number_follow_python",
        indoc! {"
            inf = 1e308 * 10
            for x in [0.0, -0.0, 1.0, 1.5, -2.5, 0.1, 1 / 3, 1e308, 2.2250738585072014e-308, 5e-324, 1e-310, inf, -inf, inf - inf]:
                print(x.hex())
            print((7).conjugate(), (-2.5).conjugate(), (2.0 ** 1023).hex(), (2.0 ** -1022).hex())
        "},
    );
}

#[test]
fn the_parts_of_a_number_follow_python() {
    same_as_cpython(
        "the_parts_of_a_number_follow_python",
        indoc! {"
            n = -7
            x = 2.5
            print(n.real, n.imag, n.numerator, n.denominator)
            print(x.real, x.imag, (-0.0).real, (1e308 * 10).imag, n.real + x.imag)
        "},
    );
}

#[test]
fn a_method_of_a_number_as_a_value_is_unsupported() {
    let source = indoc! {"
        x = 2.5
        f = x.hex
        print(f())
    "};
    assert_eq!(
        unsupported_at(source),
        ("this attribute".to_owned(), "x.hex")
    );
}

#[test]
fn a_method_through_its_type_follows_python() {
    same_as_cpython(
        "a_method_through_its_type_follows_python",
        indoc! {"
            xs = [3, 1]
            list.append(xs, 2)
            print(float.hex(1.5), str.upper('a'), int.bit_length(255), str.split('a b', sep=' '), xs)
        "},
    );
}

#[test]
fn fromhex_follows_python() {
    same_as_cpython(
        "fromhex_follows_python",
        indoc! {"
            for s in ['0x1.8p+0', '1.8p0', ' -0X1P-1 ', '0x.8', '0x1.', 'a', '0', '-0x0p+99999999999', '0x0.0000000000001p-1022']:
                print(float.fromhex(s))
            for s in ['inf', '-Infinity', 'nan', '+0x1p1023', '0x1.fffffffffffffp+1023', '0x1p-1074', '0x1p-1075', '0x1.8p-1075']:
                print(float.fromhex(s))
            for s in ['0x1.00000000000008p0', '0x1.00000000000018p0', '0x1.000000000000080001p0', '0x3p-1076', '0x1.0000000000000fp0']:
                print(float.fromhex(s))
            print(float.fromhex((0.1).hex()) == 0.1, float.fromhex('0x' + '0' * 50 + '1p0'), float.fromhex('0x1p-99999999999'))
            print(float.fromhex('0x0.' + '0' * 3000 + '1p12004'), float.fromhex('0x' + 'f' * 300 + 'p-1204'))
        "},
    );
}

#[test]
fn fromhex_raises_as_python() {
    for (i, source) in [
        "''",
        "'0x'",
        "'0x.p1'",
        "'0x1p'",
        "'0x1p+'",
        "'1g'",
        "'0x1 2'",
        "'0x1.2.3'",
        "'\\u0661'",
        "'0x1p\\u0661'",
        "'0x_1'",
        "'0x1p1024'",
        "'0x1.fffffffffffff8p1023'",
        "'0x1p99999999999'",
    ]
    .into_iter()
    .enumerate()
    {
        same_as_cpython(
            &format!("fromhex_raises_as_python_{i}"),
            &format!("print(float.fromhex({source}))"),
        );
    }
}
