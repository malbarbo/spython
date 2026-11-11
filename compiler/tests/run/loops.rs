//! The `for` loops.

use indoc::indoc;

use super::doctests::same_as_the_runner;
use super::{same_as_cpython, unsupported};

#[test]
fn for_takes_each_item_of_a_list_and_each_code_point_of_a_str() {
    same_as_cpython(
        "for_takes_each_item_of_a_list_and_each_code_point_of_a_str",
        indoc! {"
            def total(xs: list[int]) -> int:
                soma = 0
                for x in xs:
                    soma += x
                return soma


            def vogais(s: str) -> int:
                n = 0
                for c in s:
                    if c in 'aeiou':
                        n += 1
                return n


            print(total([1, 2, 3]), total([]), vogais('programação'), vogais(''))
            for s in ['a', 'bc']:
                for c in s + 'ç':
                    print(s, c)
            ws: list[list[int]] = [[1, 2], [], [3]]
            for w in ws:
                print(len(w), w)
            for b in [True, False]:
                print(b, not b)
            for f in [1.5, -0.0]:
                print(f)
            us: list[int | None] = [1, None]
            for u in us:
                if u is not None:
                    print(u + 1)
                else:
                    print(u)
        "},
    );
}

#[test]
fn break_continue_and_else_of_a_for() {
    same_as_cpython(
        "break_continue_and_else_of_a_for",
        indoc! {"
            def busca(xs: list[int], v: int) -> int:
                i = 0
                for x in xs:
                    if x == v:
                        break
                    i += 1
                else:
                    print('não achou', v)
                    return -1
                return i


            print(busca([4, 5, 6], 5), busca([4, 5, 6], 7), busca([], 1))
            for c in 'abcde':
                if c == 'b':
                    continue
                if c == 'd':
                    break
                print(c)
            else:
                print('fim')
            for c in 'xy':
                print(c)
            else:
                print('fim')
            for x in [1, 2]:
                for y in [3, 4]:
                    if y == 4:
                        break
                    print(x, y)
                else:
                    print('nunca')
        "},
    );
}

#[test]
fn the_list_of_a_for_is_the_one_before_the_loop_with_its_length_at_each_test() {
    same_as_cpython(
        "the_list_of_a_for_is_the_one_before_the_loop_with_its_length_at_each_test",
        indoc! {"
            def lista(xs: list[int]) -> list[int]:
                print('lista')
                return xs


            xs = [1, 2, 3]
            for x in lista(xs):
                if x < 3:
                    xs.append(x * 10)
                xs[0] = 7
                print(x)
            ys = [1, 2]
            for y in ys:
                ys = [5, 6, 7]
                print(y)
            print(ys, xs)
            s = 'ab'
            for c in s:
                s = s + c
                print(c)
            print(s)
        "},
    );
}

/// The checker rejects a read of the variable after the loop in the same
/// scope, but not in a function.
#[test]
fn the_variable_of_an_empty_loop_has_no_value() {
    same_as_cpython(
        "the_variable_of_an_empty_loop_has_no_value",
        indoc! {"
            def ultimo() -> int:
                return x


            xs: list[int] = []
            for x in xs:
                print(x)
            print(ultimo())
        "},
    );
}

#[test]
fn a_for_in_an_example_assigns_its_variable() {
    same_as_the_runner(
        "a_for_in_an_example_assigns_its_variable",
        indoc! {r#"
            def soma(xs: list[int]) -> int:
                """
                >>> for x in [1, 2]:
                ...     print(soma([x, x]))
                2
                4
                >>> x
                2
                """
                s = 0
                for x in xs:
                    s += x
                return s
        "#},
    );
}

#[test]
fn what_a_for_lacks_yet_is_unsupported() {
    for (source, what) in [
        ("for a, b in [[1, 2]]:\n    pass\n", "this target"),
        ("xs = [1]\nfor xs[0] in [2]:\n    pass\n", "this target"),
    ] {
        assert_eq!(unsupported(source), what, "{source}");
    }
}
