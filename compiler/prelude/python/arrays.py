"""The arrays of the prelude, which hold the items of a `list`, the table of a `dict` and of a
`set`, and their entries."""

from __spython_prelude__.intrinsics import Array, array_copy, array_len, array_new


def copied[T](a: Array[T]) -> Array[T]:
    """A new array with the items of `a`."""
    b: Array[T] = array_new(array_len(a))
    array_copy(b, 0, a, 0, array_len(a))
    return b


def resized[T](a: Array[T], length: int) -> Array[T]:
    """A new array of `length` items, the first of them the items of `a` that fit."""
    b: Array[T] = array_new(length)
    array_copy(b, 0, a, 0, min(length, array_len(a)))
    return b
