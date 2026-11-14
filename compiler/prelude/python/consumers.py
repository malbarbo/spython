"""The builtins that consume a list: `sum`, `min`, `max`, `any` and `all`. Each one reads the
length of the list at each turn, as the iterator of a list does."""

# The checker of spython takes `Callable` from `typing` alone.
from typing import Callable  # noqa: UP035

from __spython_prelude__.intrinsics import Ordered
from __spython_prelude__.math import isfinite


def sum_int[T: int](xs: list[T]) -> int:
    """`sum(xs)` of a list of `int` or of `bool`, which tests the 31 bits at each step, as `+`
    does."""
    total = 0
    for x in xs:
        total = total + x
    return total


def sum_float(xs: list[float]) -> float | int:
    """`sum(xs)` of a list of `float`, which starts at the `int` 0, as CPython does, and so
    gives 0 for an empty list. The sum goes on with the compensation of Neumaier, as in
    CPython 3.12."""
    if len(xs) == 0:
        return 0
    total = 0 + xs[0]
    c = 0.0
    first = True
    for x in xs:
        if first:
            first = False
            continue
        t = total + x
        if abs(total) >= abs(x):
            c += (total - t) + x
        else:
            c += (x - t) + total
        total = t
    # The compensation keeps the sign of a negative sum, and does not turn an infinite sum into
    # a NaN.
    if c != 0.0 and isfinite(c):
        total += c
    return total


def min_of[T: Ordered](xs: list[T]) -> T:
    """`min(xs)`, the first item that no later item is less than."""
    if len(xs) == 0:
        raise ValueError("min() iterable argument is empty")
    best = xs[0]
    first = True
    for x in xs:
        if first:
            first = False
        # The compiler takes `min(best, x)` of no object, and T may be a class.
        elif x < best:  # noqa: PLR1730
            best = x
    return best


def max_of[T: Ordered](xs: list[T]) -> T:
    """`max(xs)`, the first item that no later item is greater than."""
    if len(xs) == 0:
        raise ValueError("max() iterable argument is empty")
    best = xs[0]
    first = True
    for x in xs:
        if first:
            first = False
        # The compiler takes `max(best, x)` of no object, and T may be a class.
        elif x > best:  # noqa: PLR1730
            best = x
    return best


def min_key[T, K: Ordered](xs: list[T], key: Callable[[T], K]) -> T:
    """`min(xs, key=key)`, the first item whose key no later key is less than. `key` runs once
    for each item, before the comparison of its result."""
    if len(xs) == 0:
        raise ValueError("min() iterable argument is empty")
    best = xs[0]
    best_key = key(best)
    first = True
    for x in xs:
        if first:
            first = False
            continue
        k = key(x)
        if k < best_key:
            best = x
            best_key = k
    return best


def max_key[T, K: Ordered](xs: list[T], key: Callable[[T], K]) -> T:
    """`max(xs, key=key)`, the first item whose key no later key is greater than."""
    if len(xs) == 0:
        raise ValueError("max() iterable argument is empty")
    best = xs[0]
    best_key = key(best)
    first = True
    for x in xs:
        if first:
            first = False
            continue
        k = key(x)
        if k > best_key:
            best = x
            best_key = k
    return best


def any_of[T](xs: list[T]) -> bool:
    for x in xs:
        if x:
            return True
    return False


def all_of[T](xs: list[T]) -> bool:
    for x in xs:
        if not x:
            return False
    return True
