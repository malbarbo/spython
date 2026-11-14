"""The class of `list`, whose copy for the type of the items is the class of a `list` of the
program, and the methods of `list`, which take the list first."""

from __spython_prelude__.arrays import copied, resized
from __spython_prelude__.intrinsics import (
    Array,
    array_clear,
    array_copy,
    array_get,
    array_len,
    array_new,
    array_set,
    exact,
    same,
)
from __spython_prelude__.slices import (
    slice_end,
    slice_start,
    step_count,
    step_of,
    step_start,
)


class List[T]:
    # The items, from the first one. The array may have room for more.
    items: Array[T]
    length: int

    def __init__(self) -> None:
        items: Array[T] = array_new(0)
        self.items = items
        self.length = 0

    def done(self, items: Array[T]) -> List[T]:
        """Takes `items`, the array of a literal, as the items of the list, and gives the list."""
        self.items = items
        self.length = array_len(items)
        return self

    def __len__(self) -> int:
        return self.length

    def __getitem__(self, index: int) -> T:
        n = self.length
        i = index + n if index < 0 else index
        if i < 0 or i >= n:
            raise IndexError("list index out of range")
        return array_get(self.items, i)

    def __setitem__(self, index: int, x: T) -> None:
        n = self.length
        i = index + n if index < 0 else index
        if i < 0 or i >= n:
            raise IndexError("list assignment index out of range")
        array_set(self.items, i, x)

    def __contains__(self, x: T) -> bool:
        return contains(x, self)

    def __eq__(self, other: object) -> bool:
        """Returns `True` if `other` is a list of the same type with equal items, `False`
        otherwise."""
        return eq(self, exact(self, other))

    def __repr__(self) -> str:
        return repr_of(self)

    def iter(self) -> ListIter[T]:
        return ListIter(self)


class ListIter[T]:
    """A `for` over a `list`, which reads the length of the list at each turn, as the iterator of
    a list of CPython does."""

    xs: List[T]
    # The index of the item of the last turn.
    index: int

    def __init__(self, xs: List[T]) -> None:
        self.xs = xs
        self.index = -1

    def next(self) -> bool:
        """Goes to the next item, and returns `False` past the last one."""
        i = self.index + 1
        if i >= self.xs.length:
            return False
        self.index = i
        return True

    def key(self) -> T:
        return array_get(self.xs.items, self.index)


def append[T](xs: List[T], x: T) -> None:
    n = xs.length
    if n == array_len(xs.items):
        grow(xs, n + 1)
    array_set(xs.items, n, x)
    xs.length = n + 1


def pop[T](xs: List[T], i: int | None = None) -> T:
    """Removes the item of `xs` at `i`, from the end when `i` is negative, and gives it. The last
    item goes without `i`. An empty list raises before the test of `i`, as in CPython."""
    n = xs.length
    if n == 0:
        raise IndexError("pop from empty list")
    j = n - 1 if i is None else i
    if j < 0:
        j += n
    if j < 0 or j >= n:
        raise IndexError("pop index out of range")
    x = array_get(xs.items, j)
    array_copy(xs.items, j, xs.items, j + 1, n - j - 1)
    array_clear(xs.items, n - 1, 1)
    xs.length = n - 1
    return x


def insert[T](xs: List[T], i: int, x: T) -> None:
    """Puts `x` before the item of `xs` at `i`, which goes from the end when it is negative. An
    index out of range puts `x` at the start or at the end, as a slice does."""
    n = xs.length
    i = slice_end(i, n)
    if n == array_len(xs.items):
        grow(xs, n + 1)
    array_copy(xs.items, i + 1, xs.items, i, n - i)
    array_set(xs.items, i, x)
    xs.length = n + 1


def extend[T](xs: List[T], ys: List[T]) -> None:
    """Puts the items of `ys` at the end of `xs`. The length of `ys` comes first, so
    `xs.extend(xs)` doubles `xs`."""
    n = xs.length
    m = ys.length
    if n + m > array_len(xs.items):
        grow(xs, n + m)
    array_copy(xs.items, n, ys.items, 0, m)
    xs.length = n + m


def contains[T](x: T, xs: List[T]) -> bool:
    """`x in xs`, which compares each item with `x`, with the item on the left. An item that is
    `x` itself is equal to it without the comparison."""
    i = 0
    while i < xs.length:
        y = array_get(xs.items, i)
        if same(y, x) or y == x:
            return True
        i += 1
    return False


def index[T](
    xs: List[T], x: T, start: int | None = None, stop: int | None = None
) -> int:
    """The index of the first item of `xs` equal to `x`, between `start` and `stop`, which go
    from the end when they are negative."""
    i = slice_start(start, xs.length)
    last = slice_end(stop, xs.length)
    while i < last and i < xs.length:
        y = array_get(xs.items, i)
        if same(y, x) or y == x:
            return i
        i += 1
    raise ValueError(repr(x) + " is not in list")


def count[T](xs: List[T], x: T) -> int:
    n = 0
    i = 0
    while i < xs.length:
        y = array_get(xs.items, i)
        if same(y, x) or y == x:
            n += 1
        i += 1
    return n


def remove[T](xs: List[T], x: T) -> None:
    """Removes the first item of `xs` equal to `x`."""
    i = 0
    while i < xs.length:
        y = array_get(xs.items, i)
        if same(y, x) or y == x:
            pop(xs, i)
            return
        i += 1
    raise ValueError("list.remove(x): x not in list")


def clear[T](xs: List[T]) -> None:
    """Removes every item of `xs`, and drops its array, so the items may go."""
    items: Array[T] = array_new(0)
    xs.items = items
    xs.length = 0


def copy[T](xs: List[T]) -> List[T]:
    """A new list with the items of `xs`."""
    ys: List[T] = List()
    ys.items = resized(xs.items, xs.length)
    ys.length = xs.length
    return ys


def reverse[T](xs: List[T]) -> None:
    i = 0
    j = xs.length - 1
    while i < j:
        x = array_get(xs.items, i)
        array_set(xs.items, i, array_get(xs.items, j))
        array_set(xs.items, j, x)
        i += 1
        j -= 1


def concat[T](xs: List[T], ys: List[T]) -> List[T]:
    """`xs + ys`, a new list with the items of `xs` and then the items of `ys`."""
    n = xs.length
    m = ys.length
    items: Array[T] = array_new(n + m)
    array_copy(items, 0, xs.items, 0, n)
    array_copy(items, n, ys.items, 0, m)
    zs: List[T] = List()
    return zs.done(items)


def slice_of[T](xs: List[T], start: int | None, stop: int | None) -> List[T]:
    """`xs[start:stop]`, a new list with the items of `xs` from `start` up to `stop`."""
    n = xs.length
    first = slice_start(start, n)
    last = slice_end(stop, n)
    count = last - first if first < last else 0
    items: Array[T] = array_new(count)
    if count > 0:
        array_copy(items, 0, xs.items, first, count)
    ys: List[T] = List()
    return ys.done(items)


def slice_step[T](
    xs: List[T], start: int | None, stop: int | None, step: int | None
) -> List[T]:
    """`xs[start:stop:step]`, with the bounds that CPython adjusts to the step."""
    n = xs.length
    k = step_of(step)
    first = step_start(start, n, k)
    count = step_count(first, stop, n, k)
    items: Array[T] = array_new(count)
    for i in range(count):
        array_set(items, i, array_get(xs.items, first + i * k))
    ys: List[T] = List()
    return ys.done(items)


def grow[T](xs: List[T], need: int) -> None:
    """Gives `xs` room for `need` items, and for at least twice the items it has room for, from
    4."""
    room = array_len(xs.items)
    # Twice the room above 536870912 does not fit in an int.
    twice = 2 * room if room < 536870912 else need
    xs.items = resized(xs.items, max(4, need, twice))


def repr_of[T](xs: List[T]) -> str:
    """The `repr` of `xs`, which is also its `str`."""
    parts: list[str] = []
    i = 0
    while i < xs.length:
        parts.append(repr(array_get(xs.items, i)))
        i += 1
    return "[" + ", ".join(parts) + "]"


def repr_or_none[T](xs: List[T] | None) -> str:
    if xs is None:
        return "None"
    return repr_of(xs)


def eq[T](xs: List[T] | None, ys: List[T] | None) -> bool:
    """`xs == ys`, which compares the items in order, with the item of `xs` on the left. Two
    items that are the same object are equal without the comparison. A comparison that changes
    a list stops the loop at the end of the shorter one, and the lengths decide, as in
    `list_richcompare`."""
    if xs is ys:
        return True
    if xs is None or ys is None:
        return False
    if xs.length != ys.length:
        return False
    i = 0
    while i < xs.length and i < ys.length:
        x = array_get(xs.items, i)
        y = array_get(ys.items, i)
        if not (same(x, y) or x == y):
            return False
        i += 1
    return xs.length == ys.length
