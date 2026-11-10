"""The functions of `math` that need no instruction of their own."""


def isnan(x: float) -> bool:
    return x != x


def isinf(x: float) -> bool:
    return abs(x) == 1e999


def isfinite(x: float) -> bool:
    return abs(x) < 1e999


def degrees(x: float) -> float:
    # 180 / pi, the factor of CPython.
    return x * 57.29577951308232


def radians(x: float) -> float:
    # pi / 180, the factor of CPython.
    return x * 0.017453292519943295


def isqrt(n: int) -> int:
    if n < 0:
        raise ValueError("isqrt() argument must be nonnegative")
    if n == 0:
        return 0
    # The method of Newton, which goes down to the root from a value that
    # is not below it.
    x = n // 2 + 1
    y = (x + n // x) // 2
    while y < x:
        x = y
        y = (x + n // x) // 2
    return x


def factorial(n: int) -> int:
    if n < 0:
        raise ValueError("factorial() not defined for negative values")
    # CPython raises this error for an `n` above `2^63 - 1`. The greatest
    # factorial that fits in an `int` of spython is `12!`.
    if n > 12:
        raise OverflowError("factorial() argument should not exceed 12")
    result = 1
    while n > 1:
        result = result * n
        n = n - 1
    return result
