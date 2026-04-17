mod testlib;

use indoc::indoc;
use testlib::{run, run_expect_trap};

// =====================================================================
// Arithmetic: int × int
// =====================================================================

#[test]
fn int_add() {
    run("assert 3 + 4 == 7");
}

#[test]
fn int_sub() {
    run("assert 10 - 3 == 7");
}

#[test]
fn int_mul() {
    run("assert 6 * 7 == 42");
}

#[test]
fn int_floordiv() {
    run("assert 17 // 5 == 3");
}

#[test]
fn int_mod() {
    run("assert 17 % 5 == 2");
}

#[test]
fn int_truediv() {
    run("assert 10 / 4 == 2.5");
}

#[test]
fn int_neg() {
    run("assert -42 == 0 - 42");
}

// =====================================================================
// Arithmetic: float × float
// =====================================================================

#[test]
fn float_add() {
    run("assert 1.5 + 2.5 == 4.0");
}

#[test]
fn float_sub() {
    run("assert 5.0 - 1.5 == 3.5");
}

#[test]
fn float_mul() {
    run("assert 2.5 * 4.0 == 10.0");
}

#[test]
fn float_truediv() {
    run("assert 10.0 / 4.0 == 2.5");
}

#[test]
fn float_floordiv() {
    run("assert 7.5 // 2.0 == 3.0");
}

#[test]
fn float_neg() {
    run("assert -3.14 == 0.0 - 3.14");
}

// =====================================================================
// Arithmetic: int × float (mixed — promotes int to float)
// =====================================================================

#[test]
fn int_float_add() {
    run(indoc! {"
        def f(a: int, b: float) -> float:
            return a + b
        assert f(1, 2.5) == 3.5
    "});
}

#[test]
fn float_int_add() {
    run(indoc! {"
        def f(a: float, b: int) -> float:
            return a + b
        assert f(2.5, 1) == 3.5
    "});
}

#[test]
fn int_float_sub() {
    run(indoc! {"
        def f(a: int, b: float) -> float:
            return a - b
        assert f(5, 1.5) == 3.5
    "});
}

#[test]
fn int_float_mul() {
    run(indoc! {"
        def f(a: int, b: float) -> float:
            return a * b
        assert f(3, 2.5) == 7.5
    "});
}

#[test]
fn int_float_truediv() {
    run(indoc! {"
        def f(a: int, b: float) -> float:
            return a / b
        assert f(5, 2.0) == 2.5
    "});
}

#[test]
fn int_float_floordiv() {
    run(indoc! {"
        def f(a: int, b: float) -> float:
            return a // b
        assert f(7, 2.0) == 3.0
    "});
}

// =====================================================================
// Comparisons: int × int
// =====================================================================

#[test]
fn int_eq() {
    run(indoc! {"
        assert 1 == 1
        assert not (1 == 2)
    "});
}

#[test]
fn int_ne() {
    run(indoc! {"
        assert 1 != 2
        assert not (1 != 1)
    "});
}

#[test]
fn int_lt() {
    run(indoc! {"
        assert 1 < 2
        assert not (2 < 1)
        assert not (1 < 1)
    "});
}

#[test]
fn int_le() {
    run(indoc! {"
        assert 1 <= 2
        assert 1 <= 1
        assert not (2 <= 1)
    "});
}

#[test]
fn int_gt() {
    run(indoc! {"
        assert 2 > 1
        assert not (1 > 2)
        assert not (1 > 1)
    "});
}

#[test]
fn int_ge() {
    run(indoc! {"
        assert 2 >= 1
        assert 1 >= 1
        assert not (1 >= 2)
    "});
}

// =====================================================================
// Comparisons: float × float
// =====================================================================

#[test]
fn float_eq() {
    run(indoc! {"
        assert 1.5 == 1.5
        assert not (1.5 == 2.5)
    "});
}

#[test]
fn float_lt() {
    run(indoc! {"
        assert 1.0 < 2.0
        assert not (2.0 < 1.0)
    "});
}

// =====================================================================
// Comparisons: int × float (mixed)
// =====================================================================

#[test]
fn int_float_eq() {
    run(indoc! {"
        def f(a: int, b: float) -> bool:
            return a == b
        assert f(2, 2.0)
        assert not f(2, 2.5)
    "});
}

#[test]
fn int_float_lt() {
    run(indoc! {"
        def f(a: int, b: float) -> bool:
            return a < b
        assert f(1, 1.5)
        assert not f(2, 1.5)
    "});
}

// =====================================================================
// Boolean operations
// =====================================================================

#[test]
fn bool_and() {
    run(indoc! {"
        assert True and True
        assert not (True and False)
        assert not (False and True)
        assert not (False and False)
    "});
}

#[test]
fn bool_or() {
    run(indoc! {"
        assert True or True
        assert True or False
        assert False or True
        assert not (False or False)
    "});
}

#[test]
fn bool_not() {
    run(indoc! {"
        assert not False
        assert not (not True)
    "});
}

// =====================================================================
// Bool as int
// =====================================================================

#[test]
fn bool_arithmetic() {
    run(indoc! {"
        assert True + 1 == 2
        assert False + 1 == 1
        assert True * 5 == 5
        assert False * 5 == 0
        assert True + True == 2
    "});
}

#[test]
fn bool_comparison_with_int() {
    run(indoc! {"
        assert True == 1
        assert False == 0
        assert True != 0
        assert False != 1
    "});
}

// =====================================================================
// Functions
// =====================================================================

#[test]
fn identity_int() {
    run(indoc! {"
        def f(x: int) -> int:
            return x
        assert f(42) == 42
    "});
}

#[test]
fn call_chain() {
    run(indoc! {"
        def double(x: int) -> int:
            return x + x
        def quadruple(x: int) -> int:
            return double(double(x))
        assert quadruple(3) == 12
    "});
}

#[test]
fn local_variable() {
    run(indoc! {"
        def f(x: int) -> int:
            y: int = x + 1
            return y * 2
        assert f(5) == 12
    "});
}

#[test]
fn multiple_locals() {
    run(indoc! {"
        def f(a: int, b: int) -> int:
            sum: int = a + b
            diff: int = a - b
            return sum * diff
        assert f(5, 3) == 16
    "});
}

#[test]
fn local_float_from_int() {
    run(indoc! {"
        def f(x: int) -> float:
            y: float = x + 1
            return y
        assert f(5) == 6.0
    "});
}

// =====================================================================
// Assert failure
// =====================================================================

#[test]
fn assert_failure_traps() {
    run_expect_trap("assert 1 == 2");
}

#[test]
fn assert_false_traps() {
    run_expect_trap("assert False");
}

// =====================================================================
// Operator precedence (handled by parser, verified here)
// =====================================================================

#[test]
fn precedence_mul_before_add() {
    run("assert 2 + 3 * 4 == 14");
}

#[test]
fn precedence_div_before_sub() {
    run("assert 10 - 6 // 2 == 7");
}

#[test]
fn precedence_parens_override() {
    run("assert (2 + 3) * 4 == 20");
}

#[test]
fn precedence_comparison_before_bool() {
    run(indoc! {"
        assert 1 < 2 and 3 < 4
        assert not (1 > 2 and 3 < 4)
        assert 1 > 2 or 3 < 4
    "});
}

#[test]
fn precedence_not_before_and() {
    run(indoc! {"
        assert not False and True
        assert not (not True and True)
    "});
}

#[test]
fn precedence_and_before_or() {
    run(indoc! {"
        assert True or False and False
        assert not (False or False and True)
    "});
}

#[test]
fn precedence_unary_neg() {
    run("assert -2 * 3 == -6");
}

// =====================================================================
// Float modulo
// =====================================================================

#[test]
fn float_mod() {
    run(indoc! {"
        assert 7.5 % 2.0 == 1.5
        assert 10.0 % 3.0 == 1.0
    "});
}

#[test]
fn int_float_mod() {
    run(indoc! {"
        def f(a: int, b: float) -> float:
            return a % b
        assert f(7, 2.0) == 1.0
    "});
}

// =====================================================================
// Chained comparisons
// =====================================================================

#[test]
fn chained_lt() {
    run(indoc! {"
        assert 1 < 2 < 3
        assert not (1 < 3 < 2)
        assert not (3 < 2 < 1)
    "});
}

#[test]
fn chained_le() {
    run(indoc! {"
        assert 1 <= 1 <= 2
        assert not (1 <= 2 <= 0)
    "});
}

#[test]
fn chained_mixed() {
    run(indoc! {"
        assert 1 < 2 <= 2
        assert 0 <= 0 < 1
    "});
}

