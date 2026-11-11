"""The methods of `list`."""

from __spython_prelude__.intrinsics import (
    copy_items,
    item,
    list_capacity,
    list_reserve,
    list_set_len,
    set_item,
)


def append[T](xs: list[T], x: T) -> None:
    n = len(xs)
    if n == list_capacity(xs):
        grow(xs, n + 1)
    set_item(xs, n, x)
    list_set_len(xs, n + 1)


def pop[T](xs: list[T], i: int | None = None) -> T:
    """Removes the item of `xs` at `i`, from the end when `i` is negative, and gives it. The last
    item goes without `i`. An empty list raises before the test of `i`, as in CPython."""
    n = len(xs)
    if n == 0:
        raise IndexError("pop from empty list")
    j = n - 1 if i is None else i
    if j < 0:
        j += n
    if j < 0 or j >= n:
        raise IndexError("pop index out of range")
    x = item(xs, j)
    copy_items(xs, j, xs, j + 1, n - j - 1)
    list_set_len(xs, n - 1)
    return x


def insert[T](xs: list[T], i: int, x: T) -> None:
    """Puts `x` before the item of `xs` at `i`, which goes from the end when it is negative. An
    index out of range puts `x` at the start or at the end, as a slice does."""
    n = len(xs)
    if i < 0:
        i = max(i + n, 0)
    elif i > n:
        i = n
    if n == list_capacity(xs):
        grow(xs, n + 1)
    copy_items(xs, i + 1, xs, i, n - i)
    set_item(xs, i, x)
    list_set_len(xs, n + 1)


def extend[T](xs: list[T], ys: list[T]) -> None:
    """Puts the items of `ys` at the end of `xs`. The length of `ys` comes first, so
    `xs.extend(xs)` doubles `xs`."""
    n = len(xs)
    m = len(ys)
    if n + m > list_capacity(xs):
        grow(xs, n + m)
    copy_items(xs, n, ys, 0, m)
    list_set_len(xs, n + m)


def grow[T](xs: list[T], need: int) -> None:
    """Gives `xs` room for `need` items, and for at least twice the items it has room for, from
    4."""
    room = list_capacity(xs)
    # Twice the room above 536870912 does not fit in an int.
    twice = 2 * room if room < 536870912 else need
    list_reserve(xs, max(4, need, twice))


def repr_of[T](xs: list[T]) -> str:
    """The `repr` of `xs`, which is also its `str`."""
    parts: list[str] = []
    i = 0
    while i < len(xs):
        parts.append(repr(item(xs, i)))
        i += 1
    return "[" + ", ".join(parts) + "]"


def repr_or_none[T](xs: list[T] | None) -> str:
    if xs is None:
        return "None"
    return repr_of(xs)


def eq[T](xs: list[T] | None, ys: list[T] | None) -> bool:
    """`xs == ys`, which compares the items in order, with the item of `xs` on the left."""
    if xs is ys:
        return True
    if xs is None or ys is None:
        return False
    if len(xs) != len(ys):
        return False
    for i in range(len(xs)):
        if not item(xs, i) == item(ys, i):
            return False
    return True
