"""The methods of `list`."""

from __spython_prelude__.intrinsics import list_capacity, list_reserve, list_set_len, set_item


def append[T](xs: list[T], x: T) -> None:
    n = len(xs)
    if n == list_capacity(xs):
        grow(xs, n + 1)
    set_item(xs, n, x)
    list_set_len(xs, n + 1)


def grow[T](xs: list[T], need: int) -> None:
    """Gives `xs` room for `need` items, and for at least twice the items it has room for, from
    4."""
    room = list_capacity(xs)
    # Twice the room above 536870912 does not fit in an int.
    twice = 2 * room if room < 536870912 else need
    list_reserve(xs, max(4, need, twice))
