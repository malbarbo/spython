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
            n: list[int] | None = None
            print(n is None, bool(n))
            n = xs
            print(n is xs, n is not None, bool(n))
            n = ys
            print(bool(n), len(xs or ys))
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
        ("xs = [1]\nprint(xs)\n", "print of a list"),
        ("xs = [1]\ns = repr(xs)\n", "the repr of a list"),
        ("xs = [1]\ns = str(xs)\n", "str of a list"),
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
