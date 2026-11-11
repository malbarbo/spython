//! The lists.

use indoc::indoc;

use super::{same_as_cpython, unsupported};

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
        ("xs = [1]\nb = xs == [2]\n", "a comparison of lists"),
        ("x: list[int] | str = [1]\n", "a variable of this type"),
        ("xs: list[None] = [None]\n", "a list of this type"),
        ("xs = [1]\nxs.reverse()\n", "a call of this method"),
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
