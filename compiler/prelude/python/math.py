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
