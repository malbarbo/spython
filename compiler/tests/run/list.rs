//! The lists.

use indoc::indoc;

use super::{run, same_as_cpython, unsupported};

#[test]
fn a_list_has_a_length_an_identity_and_a_truth() {
    same_as_cpython(
        "a_list_has_a_length_an_identity_and_a_truth",
        indoc! {"
            def size(xs: list[int]) -> int:
                return len(xs)


            def empty() -> list[str]:
                return []


            xs = [1, 2, 3]
            ys: list[int] = []
            zs: list[bool] = [True, False]
            ws: list[list[int]] = [[1, 0], [0, 1], []]
            ss = ['a', 'b']
            fs = [1.5]
            print(len(xs), len(ys), len(zs), len(ws), len(ss), len(fs), len(empty()))
            print(xs is xs, xs is ys, xs is not ys, bool(xs), bool(ys), not ys, size([4]))
            if xs and not ys:
                print('xs')
            def pick(i: int) -> list[int] | None:
                if i == 0:
                    return None
                return xs if i == 1 else ys
            print(pick(0) is None, bool(pick(0)), bool(pick(1)), bool(pick(2)), pick(1) is xs)
            print(len(xs or ys))
        "},
    );
}

#[test]
fn a_literal_evaluates_its_items_in_order() {
    same_as_cpython(
        "a_literal_evaluates_its_items_in_order",
        indoc! {"
            def item(n: int) -> int:
                print(n)
                return n


            short = [item(1), item(2), item(3)]
            print(len(short))
        "},
    );
}

#[test]
fn a_literal_of_many_items_sets_each_item() {
    let items: Vec<String> = (0..70).map(|i| i.to_string()).collect();
    let source = format!(
        "def item(n: int) -> int:\n    print(n)\n    return n\n\n\nxs = [{}, item(70)]\nprint(len(xs))\n",
        items.join(", ")
    );
    same_as_cpython("a_literal_of_many_items_sets_each_item", &source);
}

#[test]
fn what_a_list_lacks_yet_is_unsupported() {
    for (source, what) in [
        ("x: list[int] | str = [1]\n", "a variable of this type"),
        ("xs: list[None] = [None]\n", "a list of this type"),
        (
            "xs = [1]\nys = [True]\nxs.extend(ys)\n",
            "a call of a generic function with these types",
        ),
        ("xs = [1]\nys = xs[1:]\n", "a slice of a list"),
    ] {
        assert_eq!(unsupported(source), what, "{source}");
    }
}

#[test]
fn append_adds_an_item_at_the_end() {
    same_as_cpython(
        "append_adds_an_item_at_the_end",
        indoc! {"
            def fill(xs: list[int], n: int) -> None:
                i = 0
                while i < n:
                    xs.append(i)
                    i += 1


            xs: list[int] = []
            fill(xs, 1000)
            print(len(xs), xs.append(5), len(xs))
            ys = [1, 2]
            zs = ys
            zs.append(3)
            print(len(ys), ys is zs)
            ss: list[str] = []
            ss.append('a')
            fs = [1.5]
            fs.append(2.5)
            ws: list[list[int]] = []
            ws.append(xs)
            ws.append([])
            bs = [True]
            bs.append(False)
            us: list[int | None] = []
            us.append(None)
            us.append(7)
            print(len(ss), len(fs), len(ws), len(bs), len(us))
        "},
    );
}

#[test]
fn print_repr_and_str_write_a_list_as_python() {
    same_as_cpython(
        "print_repr_and_str_write_a_list_as_python",
        indoc! {r#"
            from dataclasses import dataclass
            from enum import Enum


            class Cor(Enum):
                AZUL = 1
                VERDE = 2


            @dataclass
            class P:
                x: int
                s: str | None


            xs = [1, -2, 1073741823]
            empty: list[int] = []
            nested: list[list[int]] = [[]]
            print(xs, empty, [[1, 0], [0, 1]], nested)
            print(['a', "it's", 'say "hi"', '\n'], [1.5, -0.0, 1e16], [True, False])
            print([Cor.AZUL, Cor.VERDE], [P(1, None), P(2, 'b')])
            print(repr(xs), str(xs), f'{xs} {xs!r}', len(repr(empty)))
            n: list[str] | None = None
            print(n, repr(n))
            n = ['x']
            print(n, repr(n), str(n), f'{n}')
            def maybe(b: bool) -> list[int] | None:
                return [1] if b else None
            print(maybe(True), maybe(False), repr(maybe(True)), repr(maybe(False)), str(maybe(True)))
            ws: list[list[str]] = [['a'], []]
            ws.append(['b', 'c'])
            print(ws)
        "#},
    );
}

#[test]
fn join_puts_the_str_between_the_items() {
    same_as_cpython(
        "join_puts_the_str_between_the_items",
        indoc! {"
            empty: list[str] = []
            print(repr(', '.join(['a', 'b', 'c'])), repr(''.join(['x', 'y'])), repr('-'.join(['z'])))
            print(repr('ab'.join(empty)), repr('-'.join(['', ''])), 'ç'.join(['ã', 'õ']))
        "},
    );
}

#[test]
fn print_writes_the_arguments_before_a_list_whose_repr_raises() {
    same_as_cpython(
        "print_writes_the_arguments_before_a_list_whose_repr_raises",
        indoc! {"
            from dataclasses import dataclass


            @dataclass
            class N:
                n: 'N | None'


            deep: N | None = None
            i = 0
            while i < 1100:
                deep = N(deep)
                i += 1
            ns: list[N | None] = [deep]
            print(1, 'a', ns)
        "},
    );
}

#[test]
fn an_index_reads_an_item_from_the_start_or_from_the_end() {
    same_as_cpython(
        "an_index_reads_an_item_from_the_start_or_from_the_end",
        indoc! {"
            def at(xs: list[int], i: int) -> int:
                return xs[i]


            def show(n: int) -> int:
                print('index', n)
                return n


            xs = [10, 20, 30]
            print(xs[0], xs[2], xs[-1], xs[-3], at(xs, 1), xs[show(1)])
            ws: list[list[int]] = [[1, 2], [3]]
            print(ws[0][1], ws[-1][0], ws[1], len(ws[0]))
            ss = ['a']
            print(ss[0] + 'b', [True][0], [1.5][0])
            ys: list[int | None] = [1, None]
            y = ys[0]
            if y is not None:
                print(y + 1)
            if ys[0] is not None:
                print(ys[0] + 2)
            print(ys[1] is None)
        "},
    );
}

#[test]
fn an_index_out_of_range_raises_index_error() {
    for (name, read) in [
        ("an_index_past_the_end_raises_index_error", "xs[3]"),
        ("an_index_before_the_start_raises_index_error", "xs[-4]"),
        ("an_index_of_an_inner_list_raises_index_error", "ws[0][2]"),
        ("an_index_of_an_empty_list_raises_index_error", "empty[0]"),
    ] {
        same_as_cpython(
            name,
            &format!(
                "xs = [10, 20, 30]\nws: list[list[int]] = [[1, 2]]\nempty: list[str] = []\nprint({read})\n"
            ),
        );
    }
}

#[test]
fn an_assignment_to_an_index_sets_the_item() {
    same_as_cpython(
        "an_assignment_to_an_index_sets_the_item",
        indoc! {"
            def show(s: str, n: int) -> int:
                print(s)
                return n


            def lists(xs: list[int]) -> list[int]:
                print('list')
                return xs


            xs = [10, 20, 30]
            xs[0] = 1
            xs[-1] = 3
            ys = xs
            ys[1] = 2
            print(xs)
            lists(xs)[show('index', 1)] = show('value', 5)
            print(xs)
            ws: list[list[int]] = [[1, 2], [3]]
            ws[0][1] = 7
            ws[1] = []
            ss = ['a', 'b']
            ss[1] = 'c'
            us: list[int | None] = [1, None]
            us[0] = None
            us[1] = 4
            print(ws, ss, us)
        "},
    );
}

#[test]
fn an_assignment_out_of_range_raises_index_error() {
    for (name, target) in [
        ("an_assignment_past_the_end_raises_index_error", "xs[3]"),
        (
            "an_assignment_before_the_start_raises_index_error",
            "xs[-4]",
        ),
        (
            "an_assignment_to_an_inner_list_raises_index_error",
            "ws[0][2]",
        ),
    ] {
        same_as_cpython(
            name,
            &format!("xs = [10, 20, 30]\nws: list[list[int]] = [[1, 2]]\n{target} = 5\n"),
        );
    }
}

#[test]
fn an_item_of_a_union_that_a_call_assigns_after_the_test_raises() {
    let output = run(
        "an_item_of_a_union_that_a_call_assigns_after_the_test_raises",
        indoc! {"
            def clear(xs: list[int | None]) -> None:
                xs[0] = None


            xs: list[int | None] = [1]
            if xs[0] is not None:
                clear(xs)
                print(xs[0] + 1)
        "},
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let end = indoc! {"
        \", line 8, in <module>
            print(xs[0] + 1)
                  ~~^^^
        TypeError: the item changed after the test that narrowed its type
    "};
    assert!(stderr.ends_with(end), "{stderr}");
}

#[test]
fn equal_lists_have_equal_items_in_the_same_order() {
    same_as_cpython(
        "equal_lists_have_equal_items_in_the_same_order",
        indoc! {"
            from dataclasses import dataclass
            from enum import Enum


            class Cor(Enum):
                AZUL = 1
                VERDE = 2


            @dataclass
            class P:
                x: int
                s: str | None


            def pick(b: bool) -> list[int] | None:
                return [1, 2] if b else None


            xs = [1, 2, 3]
            ys = xs
            empty: list[int] = []
            other: list[int] = []
            print(xs == [1, 2, 3], xs != [1, 2, 3], xs == [1, 2], xs == [3, 2, 1], xs == ys, xs != ys)
            print(empty == other, empty == xs, xs == empty)
            ws: list[list[int]] = [[1], []]
            print(ws == [[1], other], [[1], [2]] == [[1], [3]], ['a', 'b'] == ['a', 'b'], ['a'] == ['b'])
            print([True] == [True], [True] == [False], [1.5, -0.0] == [1.5, 0.0], [0.1 + 0.2] == [0.3])
            print([Cor.AZUL] == [Cor.AZUL], [Cor.AZUL] == [Cor.VERDE])
            print([P(1, None)] == [P(1, None)], [P(1, 'a')] == [P(1, 'b')])
            ns: list[int | None] = [1, None]
            print(ns == [1, None], ns == [None, 1])
            ss: list[str | None] = ['a', None]
            print(ss == ['a', None], ss == [None, 'a'])
            ps: list[P | None] = [None, P(2, None)]
            print(ps == [None, P(2, None)], ps == [P(2, None), None])
            print(pick(True) == [1, 2], pick(False) == [1, 2], [2, 1] != pick(True))
            print(pick(False) == pick(False), pick(True) == pick(True), [1, 2] == pick(True))
        "},
    );
}

#[test]
fn a_comparison_of_lists_that_spython_lacks_is_unsupported() {
    for (source, what) in [
        ("b = [1] == [True]\n", "a comparison of two types of list"),
        (
            "xs: list[int | None] = [1]\nb = xs == [1]\n",
            "a comparison of two types of list",
        ),
        ("b = [1] < [2]\n", "an order of lists"),
        (
            "xs = [1]\nb = xs == xs == xs\n",
            "a chain of comparisons of lists",
        ),
        (
            indoc! {"
                from dataclasses import dataclass


                @dataclass
                class P:
                    x: int


                xs: list[list[P | int]] = [[P(1)]]
                b = xs == xs
            "},
            "== of a list whose items may be dataclasses of a union",
        ),
    ] {
        assert_eq!(unsupported(source), what, "{source}");
    }
}

#[test]
fn pop_insert_and_extend_change_the_list_in_place() {
    same_as_cpython(
        "pop_insert_and_extend_change_the_list_in_place",
        indoc! {"
            xs = [1, 2, 3, 4, 5]
            print(xs.pop(), xs, xs.pop(0), xs, xs.pop(-2), xs, xs.pop(1), xs)
            ys = xs
            print(ys.pop(), xs, len(xs))
            for i in [0, -1, 10, -10, 1, 2]:
                xs.insert(i, i)
                print(xs)
            empty: list[str] = []
            empty.insert(5, 'a')
            empty.insert(-5, 'b')
            print(empty)
            zs: list[int] = []
            i = 0
            while i < 100:
                zs.insert(0, i)
                i += 1
            print(len(zs), zs[0], zs[-1], zs.pop(50))
            ws: list[int] = [1, 2]
            ws.extend([3, 4])
            ws.extend(ws)
            other: list[int] = []
            ws.extend(other)
            other.extend(ws)
            print(ws, other)
            bs = [True]
            bs.extend([False])
            bs.insert(0, False)
            print(bs, bs.pop())
            ls: list[list[int]] = [[1]]
            ls.extend([[2], []])
            print(ls.pop(), ls)
            vs = [1, 2, 3, 4]
            for v in vs:
                print(v, vs.pop())
            print(vs)
        "},
    );
}

#[test]
fn pop_of_an_empty_list_or_out_of_range_raises_index_error() {
    for (name, source) in [
        (
            "pop_of_an_empty_list_raises_index_error",
            "xs: list[int] = []\nxs.pop()\n",
        ),
        (
            "pop_of_an_empty_list_with_an_index_raises_index_error",
            "xs: list[int] = []\nprint(xs.pop(5))\n",
        ),
        (
            "pop_past_the_end_raises_index_error",
            "xs = [1]\nxs.pop(1)\n",
        ),
        (
            "pop_before_the_start_raises_index_error",
            "xs = [1, 2]\nxs.pop(-3)\n",
        ),
    ] {
        same_as_cpython(name, source);
    }
}

#[test]
fn in_index_count_and_remove_look_for_an_equal_item() {
    same_as_cpython(
        "in_index_count_and_remove_look_for_an_equal_item",
        indoc! {"
            from dataclasses import dataclass


            @dataclass
            class P:
                x: int
                y: int


            def show(s: str, n: int) -> int:
                print(s)
                return n


            def lista(xs: list[int]) -> list[int]:
                print('lista')
                return xs


            xs = [1, 2, 3, 2, 1]
            print(2 in xs, 5 in xs, 5 not in xs, 2 not in xs, show('x', 3) in lista(xs))
            print(xs.index(2), xs.index(1, 1), xs.index(2, -2), xs.index(2, 0, 2), xs.index(3, -10, 10))
            print(xs.count(1), xs.count(3), xs.count(7), [[1], [2], [1]].count([1]))
            xs.remove(2)
            print(xs)
            xs.remove(1)
            xs.remove(1)
            print(xs, 'a' in ['a', 'b'], 'c' in ['a'], [1] in [[0], [1]], True in [False])
            ps = [P(1, 2), P(3, 4)]
            print(P(3, 4) in ps, P(4, 3) in ps, ps.index(P(3, 4)), ps.count(P(1, 2)))
            ps.remove(P(1, 2))
            print(ps)
            ns: list[str | None] = ['a', None]
            print(None in ns, 'a' in ns, ns.index(None), ns.count('b'))
            empty: list[int] = []
            print(1 in empty, empty.count(1))
        "},
    );
}

#[test]
fn index_and_remove_of_a_missing_item_raise_value_error() {
    for (name, source) in [
        (
            "index_of_a_missing_item_raises_value_error",
            indoc! {"
                xs = [1]
                xs.index(9)
            "},
        ),
        (
            "index_of_a_missing_str_raises_value_error_with_its_repr",
            indoc! {"
                xs = ['a']
                print(xs.index('b'))
            "},
        ),
        (
            "index_of_an_item_out_of_the_range_raises_value_error",
            indoc! {"
                xs = [1, 2]
                xs.index(1, 1)
            "},
        ),
        (
            "remove_of_a_missing_item_raises_value_error",
            indoc! {"
                xs = [1]
                xs.remove(9)
            "},
        ),
    ] {
        same_as_cpython(name, source);
    }
}

#[test]
fn a_search_in_a_list_of_a_union_with_a_dataclass_is_unsupported() {
    let class = indoc! {"
        from dataclasses import dataclass


        @dataclass
        class P:
            x: int


        xs: list[P | int] = [P(1)]
    "};
    for search in [
        "b = 1 in xs",
        "n = xs.count(1)",
        "n = xs.index(1)",
        "xs.remove(1)",
    ] {
        assert_eq!(
            unsupported(&format!("{class}{search}\n")),
            "a search in a list whose items may be dataclasses of a union",
            "{search}"
        );
    }
}

#[test]
fn clear_copy_and_reverse_of_a_list() {
    same_as_cpython(
        "clear_copy_and_reverse_of_a_list",
        indoc! {"
            xs = [1, 2, 3]
            ys = xs.copy()
            ys.append(4)
            print(xs, ys, xs is ys, xs == [1, 2, 3])
            xs.reverse()
            ys.reverse()
            print(xs, ys)
            ws: list[list[int]] = [[1], [2, 3]]
            vs = ws.copy()
            vs[0].append(5)
            print(ws, vs, ws[0] is vs[0])
            empty: list[str] = []
            print(empty.copy(), empty.copy() is empty, empty.reverse(), empty)
            zs = xs
            xs.clear()
            print(xs, zs, len(zs), xs.clear())
            xs.append(7)
            xs.extend([8, 9])
            print(zs)
            one = ['a']
            one.reverse()
            print(one)
        "},
    );
}
