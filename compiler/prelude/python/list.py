"""The methods of `list`."""

from __spython_prelude__.intrinsics import (
    copy_items,
    item,
    list_capacity,
    list_reserve,
    list_set_len,
    list_truncate,
    set_item,
)
from __spython_prelude__.slices import (
    slice_end,
    slice_start,
    step_count,
    step_of,
    step_start,
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
    list_truncate(xs, n - 1)
    return x


def insert[T](xs: list[T], i: int, x: T) -> None:
    """Puts `x` before the item of `xs` at `i`, which goes from the end when it is negative. An
    index out of range puts `x` at the start or at the end, as a slice does."""
    n = len(xs)
    i = slice_end(i, n)
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


def contains[T](x: T, xs: list[T]) -> bool:
    """`x in xs`, which compares each item with `x`, with the item on the left."""
    for i in range(len(xs)):
        if item(xs, i) == x:
            return True
    return False


def index[T](
    xs: list[T], x: T, start: int | None = None, stop: int | None = None
) -> int:
    """The index of the first item of `xs` equal to `x`, between `start` and `stop`, which go
    from the end when they are negative."""
    for i in range(slice_start(start, len(xs)), slice_end(stop, len(xs))):
        if item(xs, i) == x:
            return i
    raise ValueError(repr(x) + " is not in list")


def count[T](xs: list[T], x: T) -> int:
    n = 0
    for i in range(len(xs)):
        if item(xs, i) == x:
            n += 1
    return n


def remove[T](xs: list[T], x: T) -> None:
    """Removes the first item of `xs` equal to `x`."""
    for i in range(len(xs)):
        if item(xs, i) == x:
            pop(xs, i)
            return
    raise ValueError("list.remove(x): x not in list")


def clear[T](xs: list[T]) -> None:
    """Removes every item of `xs`, and drops its array, so the items may go."""
    list_set_len(xs, 0)
    list_reserve(xs, 0)


def copy[T](xs: list[T]) -> list[T]:
    """A new list with the items of `xs`."""
    ys: list[T] = []
    extend(ys, xs)
    return ys


def reverse[T](xs: list[T]) -> None:
    i = 0
    j = len(xs) - 1
    while i < j:
        x = item(xs, i)
        set_item(xs, i, item(xs, j))
        set_item(xs, j, x)
        i += 1
        j -= 1


def concat[T](xs: list[T], ys: list[T]) -> list[T]:
    """`xs + ys`, a new list with the items of `xs` and then the items of `ys`."""
    zs: list[T] = []
    list_reserve(zs, len(xs) + len(ys))
    extend(zs, xs)
    extend(zs, ys)
    return zs


def slice_of[T](xs: list[T], start: int | None, stop: int | None) -> list[T]:
    """`xs[start:stop]`, a new list with the items of `xs` from `start` up to `stop`."""
    n = len(xs)
    first = slice_start(start, n)
    last = slice_end(stop, n)
    ys: list[T] = []
    if first < last:
        list_reserve(ys, last - first)
        copy_items(ys, 0, xs, first, last - first)
        list_set_len(ys, last - first)
    return ys


def slice_step[T](
    xs: list[T], start: int | None, stop: int | None, step: int | None
) -> list[T]:
    """`xs[start:stop:step]`, with the bounds that CPython adjusts to the step."""
    n = len(xs)
    k = step_of(step)
    first = step_start(start, n, k)
    count = step_count(first, stop, n, k)
    ys: list[T] = []
    list_reserve(ys, count)
    for i in range(count):
        set_item(ys, i, item(xs, first + i * k))
    list_set_len(ys, count)
    return ys


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
    list_reserve(parts, len(xs))
    for i in range(len(xs)):
        parts.append(repr(item(xs, i)))
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
