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
