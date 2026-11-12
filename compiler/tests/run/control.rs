//! `not`, `and`, `or`, `if`, the conditional expression, `while`, `break`
//! and `pass`.

use indoc::indoc;

use super::{same_as_cpython, stops_at_the_int_limit, unsupported};

#[test]
fn not_negates_a_bool() {
    same_as_cpython(
        "not_negates_a_bool",
        indoc! {"
            t = True
            print(not t, not not t, not 1 > 2)
            n = 0
            while not n == 3:
                n += 1
            print(n, not t + 1 == 2)
        "},
    );
}

#[test]
fn not_takes_the_truth_of_an_int_or_a_str() {
    same_as_cpython(
        "not_takes_the_truth_of_an_int_or_a_str",
        indoc! {"
            n = 0
            s = 'a'
            print(not n, not n + 1, not -1, not s, not '', not not s)
            while not n:
                n += 1
            print(n)
        "},
    );
}

#[test]
fn none_is_false() {
    same_as_cpython(
        "none_is_false",
        indoc! {"
            print(not print(1), bool(print(0)))
            if print(2):
                print(3)
            else:
                print(4)
            while print(5):
                print(6)
        "},
    );
}

#[test]
fn and_and_or_combine_bools() {
    same_as_cpython(
        "and_and_or_combine_bools",
        indoc! {"
            t = True
            f = False
            print(t and t, t and f, f and t, f and f)
            print(t or t, t or f, f or t, f or f)
            print(t and f or t, f or t and f, not f and t, t and t and f, f or f or t)
            n = 0
            while n < 10 and not n == 4 or n == 7:
                n += 1
            print(n)
        "},
    );
}

#[test]
fn and_and_or_skip_the_right_operand_when_the_left_decides() {
    same_as_cpython(
        "and_and_or_skip_the_right_operand_when_the_left_decides",
        indoc! {"
            print(False and 1 // 0 == 0)  # ty: ignore[division-by-zero]
            print(True or 1 // 0 == 0)  # ty: ignore[division-by-zero]
            print(True and 1 % 0 == 0)  # ty: ignore[division-by-zero]
        "},
    );
}

#[test]
fn and_and_or_give_an_operand_of_int_or_str() {
    same_as_cpython(
        "and_and_or_give_an_operand_of_int_or_str",
        indoc! {"
            n = 0
            m = 5
            s = ''
            t = 'abc'
            print(n and m, m and n, m and 7, n or m, m or n, n or 0)
            print(s and t, t and s, t and 'x', s or t, t or s, s or '')
            print(n or m and 3, n and m or -1, n or n or 9)
            r = s or 'vazio'
            print(r)
            while m and m - 3:
                m -= 1
            print(m)
        "},
    );
}

#[test]
fn and_and_or_of_two_types_take_the_truth_of_each_where_only_the_truth_matters() {
    same_as_cpython(
        "and_and_or_of_two_types_take_the_truth_of_each_where_only_the_truth_matters",
        indoc! {"
            m = 3
            t = 'abc'
            while m and t:
                m -= 1
            print(m)
            if not (m or t) or m and not t:
                print('nunca')
            elif t and (m == 0 or t):
                print('sim')
            print(not (m and t), not ('' or 2), not (m or t and 1 > 2))
            print(not (0 and 1 // 0))  # ty: ignore[division-by-zero]
        "},
    );
}

/// `and` and `or` give one of the operands, which may have two types,
/// and take the truth of each operand once.
#[test]
fn and_and_or_of_two_types_give_one_of_the_operands() {
    same_as_cpython(
        "and_and_or_of_two_types_give_one_of_the_operands",
        indoc! {"
            class P:
                n: int

                def __init__(self, n: int) -> None:
                    self.n = n

                def __bool__(self) -> bool:
                    print('bool', self.n)
                    return self.n > 0

                def __repr__(self) -> str:
                    return f'P({self.n})'

            def ou(s: str | None) -> str:
                return s or 'vazio'

            def zero(n: int | None) -> int:
                return n or 0

            def nome(p: P | None) -> P:
                return p or P(9)

            t = True
            n = 0
            print(t and 1, t or 1, 'a' or n, n and 'a', n or 'b')
            print(print(1) or 1, None and 2.5, 0.0 or None)
            print(ou(None), ou(''), ou('x'))
            print(zero(None), zero(0), zero(4))
            print(nome(None), nome(P(1)))
            print(P(0) and 1, P(2) and 1, P(0) or 'x', P(3) or 'x')
            print(P(1) and P(0) and P(2), P(0) or P(-1) or 5)
            print(1 and P(1) or 'nada')
            xs = [1]
            ys: list[int] | None = None
            print(ys or xs, xs and [2] or ys)
        "},
    );
}

/// The `ValueError` of a `__len__` in `and` or `or` points at the whole
/// operator, as in CPython.
#[test]
fn an_error_in_the_truth_of_an_operand_of_and_raises_at_the_operator() {
    same_as_cpython(
        "an_error_in_the_truth_of_an_operand_of_and_raises_at_the_operator",
        indoc! {"
            class N:
                def __len__(self) -> int:
                    return -1

            def f(x: N, y: int) -> int | N:
                return y and   x    or   3

            print(f(N(), 0))
            print(f(N(), 1))
        "},
    );
}

#[test]
fn and_of_a_list_and_another_type_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            xs = [1]
            print(xs and 1)
        "}),
        "operands of two types"
    );
}

#[test]
fn an_if_runs_the_first_clause_whose_condition_is_true() {
    same_as_cpython(
        "an_if_runs_the_first_clause_whose_condition_is_true",
        indoc! {"
            n = 7
            if n > 5:
                print('maior')
            if n < 5:
                print('menor')
            if n < 5:
                print('a')
            else:
                print('b')
            if n < 0:
                s = 'negativo'
            elif n == 0:
                s = 'zero'
            elif n % 2 == 0:
                s = 'par'
            else:
                s = 'ímpar'
            print(s)
            if n > 0:
                if n > 10:
                    print('grande')
                else:
                    print('pequeno')
                n = n * 2
            print(n)
        "},
    );
}

#[test]
fn a_condition_takes_the_truth_of_an_int_or_a_str() {
    same_as_cpython(
        "a_condition_takes_the_truth_of_an_int_or_a_str",
        indoc! {"
            zero = 0
            if zero:
                print('nunca')
            else:
                print('zero')
            if -3:
                print('-3')
            if '':
                print('nunca')
            elif 'a':
                print('a')
            n = 3
            while n:
                print(n)
                n -= 1
            s = 'abc'
            while s:
                print(s)
                s = ''
            if n - 1:
                print(n - 1)
        "},
    );
}

#[test]
fn a_while_repeats_while_the_condition_is_true() {
    same_as_cpython(
        "a_while_repeats_while_the_condition_is_true",
        indoc! {"
            i = 0
            total = 0
            while i < 5:
                i += 1
                if i == 2:
                    continue
                total += i
                print(i, total)
            print(total)
            while False:
                print('nunca')
        "},
    );
}

#[test]
fn a_pass_does_nothing() {
    same_as_cpython(
        "a_pass_does_nothing",
        indoc! {"
            n = 0
            while n < 3:
                n += 1
                if n == 2:
                    pass
                else:
                    print(n)
            if n == 3:
                pass
            print('fim')
        "},
    );
}

#[test]
fn a_break_leaves_the_innermost_loop_without_its_else() {
    same_as_cpython(
        "a_break_leaves_the_innermost_loop_without_its_else",
        indoc! {"
            i = 0
            while True:
                j = 0
                while j < 10:
                    if j == 2:
                        break
                    print(i, j)
                    j += 1
                else:
                    print('nunca')
                i += 1
                if i == 3:
                    break
            print(i)
            n = 3
            while n > 0:
                n -= 1
            else:
                print('fim', n)
        "},
    );
}

#[test]
fn a_read_after_an_assignment_in_a_loop_finds_the_value() {
    same_as_cpython(
        "a_read_after_an_assignment_in_a_loop_finds_the_value",
        indoc! {"
            i = 0
            while i < 3:
                if i == 1:
                    x = 10
                i += 1
            print(x)  # ty: ignore[possibly-unresolved-reference]
            while True:
                y = 1
                break
            print(y)
        "},
    );
}

#[test]
fn a_read_without_an_assignment_in_a_loop_raises_name_error() {
    same_as_cpython(
        "a_read_without_an_assignment_in_a_loop_raises_name_error",
        indoc! {"
            i = 0
            while i < 3:
                if i == 5:
                    x = 10
                i += 1
            print(i)
            print(x)  # ty: ignore[possibly-unresolved-reference]
        "},
    );
}

#[test]
fn a_division_by_a_variable_that_is_zero_raises_zero_division_error() {
    same_as_cpython(
        "a_division_by_a_variable_that_is_zero_raises_zero_division_error",
        indoc! {"
            d = 3
            while d > 0:
                print(12 // d, 12 % d)
                d -= 1
            print(12 // d)
        "},
    );
}

#[test]
fn a_loop_that_doubles_an_int_stops_at_the_int_limit() {
    stops_at_the_int_limit(
        "a_loop_that_doubles_an_int_stops_at_the_int_limit",
        indoc! {"
            n = 1
            while True:
                n *= 2
        "},
        "",
        3,
        "    n *= 2\n",
        "1073741824",
    );
}

#[test]
fn a_conditional_expression_gives_the_value_that_the_condition_picks() {
    same_as_cpython(
        "a_conditional_expression_gives_the_value_that_the_condition_picks",
        indoc! {"
            def sign(n: int) -> str:
                return 'neg' if n < 0 else 'zero' if n == 0 else 'pos'

            def half(n: int) -> int | float:
                return n / 2 if n % 2 else n // 2

            n = 3
            print(sign(-n), sign(0), sign(n), half(3), half(4))
            print(1 if n else 2, 'a' if '' else 'b', (n if n > 5 else -n) * 2)
            print(n if n > 5 else 'small')
        "},
    );
}

#[test]
fn a_conditional_expression_runs_only_the_value_that_the_condition_picks() {
    same_as_cpython(
        "a_conditional_expression_runs_only_the_value_that_the_condition_picks",
        indoc! {"
            def shown(n: int) -> int:
                print('shown', n)
                return n

            print(shown(1) if shown(2) > 1 else shown(3))
            print(shown(1) if shown(0) else shown(3))
            print('a') if shown(4) else print('b')
        "},
    );
}

#[test]
fn a_conditional_expression_narrows_a_str_or_none() {
    same_as_cpython(
        "a_conditional_expression_narrows_a_str_or_none",
        indoc! {"
            def shout(s: str | None) -> str:
                return s.upper() if s is not None else '-'

            print(shout('oi'), shout(None))
        "},
    );
}
