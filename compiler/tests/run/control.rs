//! `not`, `and`, `or`, `if`, `while`, `break` and `pass`.

use super::{same_as_cpython, stops_at_the_int_limit, unsupported};

#[test]
fn not_negates_a_bool() {
    same_as_cpython(
        "not_negates_a_bool",
        "t = True\n\
         print(not t, not not t, not 1 > 2)\n\
         n = 0\n\
         while not n == 3:\n    n += 1\n\
         print(n, not t + 1 == 2)\n",
    );
}

#[test]
fn not_takes_the_truth_of_an_int_or_a_str() {
    same_as_cpython(
        "not_takes_the_truth_of_an_int_or_a_str",
        "n = 0\ns = 'a'\n\
         print(not n, not n + 1, not -1, not s, not '', not not s)\n\
         while not n:\n    n += 1\n\
         print(n)\n",
    );
}

#[test]
fn not_of_none_is_unsupported() {
    assert_eq!(unsupported("print(not print(1))\n"), "the truth of None");
}

#[test]
fn and_and_or_combine_bools() {
    same_as_cpython(
        "and_and_or_combine_bools",
        "t = True\nf = False\n\
         print(t and t, t and f, f and t, f and f)\n\
         print(t or t, t or f, f or t, f or f)\n\
         print(t and f or t, f or t and f, not f and t, t and t and f, f or f or t)\n\
         n = 0\n\
         while n < 10 and not n == 4 or n == 7:\n    n += 1\n\
         print(n)\n",
    );
}

#[test]
fn and_and_or_skip_the_right_operand_when_the_left_decides() {
    same_as_cpython(
        "and_and_or_skip_the_right_operand_when_the_left_decides",
        "print(False and 1 // 0 == 0)  # ty: ignore[division-by-zero]\n\
         print(True or 1 // 0 == 0)  # ty: ignore[division-by-zero]\n\
         print(True and 1 % 0 == 0)  # ty: ignore[division-by-zero]\n",
    );
}

#[test]
fn and_and_or_give_an_operand_of_int_or_str() {
    same_as_cpython(
        "and_and_or_give_an_operand_of_int_or_str",
        "n = 0\nm = 5\ns = ''\nt = 'abc'\n\
         print(n and m, m and n, m and 7, n or m, m or n, n or 0)\n\
         print(s and t, t and s, t and 'x', s or t, t or s, s or '')\n\
         print(n or m and 3, n and m or -1, n or n or 9)\n\
         r = s or 'vazio'\n\
         print(r)\n\
         while m and m - 3:\n    m -= 1\n\
         print(m)\n",
    );
}

#[test]
fn and_and_or_of_two_types_take_the_truth_of_each_where_only_the_truth_matters() {
    same_as_cpython(
        "and_and_or_of_two_types_take_the_truth_of_each_where_only_the_truth_matters",
        "m = 3\nt = 'abc'\n\
         while m and t:\n    m -= 1\n\
         print(m)\n\
         if not (m or t) or m and not t:\n    print('nunca')\nelif t and (m == 0 or t):\n    print('sim')\n\
         print(not (m and t), not ('' or 2), not (m or t and 1 > 2))\n\
         print(not (0 and 1 // 0))  # ty: ignore[division-by-zero]\n",
    );
}

#[test]
fn and_of_operands_of_two_types_is_unsupported() {
    assert_eq!(
        unsupported("t = True\nprint(t and 1)\n"),
        "operands of two types"
    );
    assert_eq!(
        unsupported("n = 0\nprint('a' or n)\n"),
        "operands of two types"
    );
    assert_eq!(unsupported("print(print(1) or 1)\n"), "the truth of None");
}

#[test]
fn an_if_runs_the_first_clause_whose_condition_is_true() {
    same_as_cpython(
        "an_if_runs_the_first_clause_whose_condition_is_true",
        "n = 7\n\
         if n > 5:\n    print('maior')\n\
         if n < 5:\n    print('menor')\n\
         if n < 5:\n    print('a')\nelse:\n    print('b')\n\
         if n < 0:\n    s = 'negativo'\nelif n == 0:\n    s = 'zero'\nelif n % 2 == 0:\n    s = 'par'\nelse:\n    s = 'ímpar'\n\
         print(s)\n\
         if n > 0:\n    if n > 10:\n        print('grande')\n    else:\n        print('pequeno')\n    n = n * 2\n\
         print(n)\n",
    );
}

#[test]
fn a_condition_takes_the_truth_of_an_int_or_a_str() {
    same_as_cpython(
        "a_condition_takes_the_truth_of_an_int_or_a_str",
        "zero = 0\n\
         if zero:\n    print('nunca')\nelse:\n    print('zero')\n\
         if -3:\n    print('-3')\n\
         if '':\n    print('nunca')\nelif 'a':\n    print('a')\n\
         n = 3\n\
         while n:\n    print(n)\n    n -= 1\n\
         s = 'abc'\n\
         while s:\n    print(s)\n    s = ''\n\
         if n - 1:\n    print(n - 1)\n",
    );
}

#[test]
fn a_condition_of_none_is_unsupported() {
    assert_eq!(
        unsupported("if print(1):\n    print(2)\n"),
        "the truth of None"
    );
}

#[test]
fn a_while_repeats_while_the_condition_is_true() {
    same_as_cpython(
        "a_while_repeats_while_the_condition_is_true",
        "i = 0\ntotal = 0\n\
         while i < 5:\n    i += 1\n    if i == 2:\n        continue\n    total += i\n    print(i, total)\n\
         print(total)\n\
         while False:\n    print('nunca')\n",
    );
}

#[test]
fn a_pass_does_nothing() {
    same_as_cpython(
        "a_pass_does_nothing",
        "n = 0\n\
         while n < 3:\n    n += 1\n    if n == 2:\n        pass\n    else:\n        print(n)\n\
         if n == 3:\n    pass\n\
         print('fim')\n",
    );
}

#[test]
fn a_break_leaves_the_innermost_loop_without_its_else() {
    same_as_cpython(
        "a_break_leaves_the_innermost_loop_without_its_else",
        "i = 0\n\
         while True:\n    j = 0\n    while j < 10:\n        if j == 2:\n            break\n        print(i, j)\n        j += 1\n    else:\n        print('nunca')\n    i += 1\n    if i == 3:\n        break\n\
         print(i)\n\
         n = 3\n\
         while n > 0:\n    n -= 1\nelse:\n    print('fim', n)\n",
    );
}

#[test]
fn a_read_after_an_assignment_in_a_loop_finds_the_value() {
    same_as_cpython(
        "a_read_after_an_assignment_in_a_loop_finds_the_value",
        "i = 0\nwhile i < 3:\n    if i == 1:\n        x = 10\n    i += 1\n\
         print(x)  # ty: ignore[possibly-unresolved-reference]\n\
         while True:\n    y = 1\n    break\nprint(y)\n",
    );
}

#[test]
fn a_read_without_an_assignment_in_a_loop_raises_name_error() {
    same_as_cpython(
        "a_read_without_an_assignment_in_a_loop_raises_name_error",
        "i = 0\nwhile i < 3:\n    if i == 5:\n        x = 10\n    i += 1\n\
         print(i)\nprint(x)  # ty: ignore[possibly-unresolved-reference]\n",
    );
}

#[test]
fn a_division_by_a_variable_that_is_zero_raises_zero_division_error() {
    same_as_cpython(
        "a_division_by_a_variable_that_is_zero_raises_zero_division_error",
        "d = 3\nwhile d > 0:\n    print(12 // d, 12 % d)\n    d -= 1\nprint(12 // d)\n",
    );
}

#[test]
fn a_loop_that_doubles_an_int_stops_at_the_int_limit() {
    stops_at_the_int_limit(
        "a_loop_that_doubles_an_int_stops_at_the_int_limit",
        "n = 1\nwhile True:\n    n *= 2\n",
        "",
        3,
        "    n *= 2\n",
        "1073741824",
    );
}
