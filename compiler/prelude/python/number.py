"""The operators on a value of `int | float`, which test the type of each
operand when the program runs."""


def as_float(x: int | float) -> float:
    if isinstance(x, int):
        return float(x)
    return x


def add(a: int | float, b: int | float) -> int | float:
    if isinstance(a, int) and isinstance(b, int):
        return a + b
    return as_float(a) + as_float(b)


def sub(a: int | float, b: int | float) -> int | float:
    if isinstance(a, int) and isinstance(b, int):
        return a - b
    return as_float(a) - as_float(b)


def mul(a: int | float, b: int | float) -> int | float:
    if isinstance(a, int) and isinstance(b, int):
        return a * b
    return as_float(a) * as_float(b)


def truediv(a: int | float, b: int | float) -> float:
    if isinstance(a, int) and isinstance(b, int):
        return a / b
    return as_float(a) / as_float(b)


def floordiv(a: int | float, b: int | float) -> int | float:
    if isinstance(a, int) and isinstance(b, int):
        return a // b
    return as_float(a) // as_float(b)


def mod(a: int | float, b: int | float) -> int | float:
    if isinstance(a, int) and isinstance(b, int):
        return a % b
    return as_float(a) % as_float(b)


def power(a: int | float, b: int) -> int | float:
    """`a ** b`. The checker rejects an exponent that may be a `float`."""
    if isinstance(a, int):
        return a**b
    return a ** float(b)


def neg(x: int | float) -> int | float:
    if isinstance(x, int):
        return -x
    return -x
