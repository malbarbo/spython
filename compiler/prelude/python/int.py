"""The operators of `int` that need no instruction of their own."""

from __spython_prelude__.intrinsics import natural_power


def power(base: int, exp: int) -> int | float:
    """`base ** exp`, which is a `float` for a negative `exp`, as in Python."""
    if exp < 0:
        return float(base) ** float(exp)
    return natural_power(base, exp)
