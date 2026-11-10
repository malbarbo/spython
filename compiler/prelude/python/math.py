"""The functions of `math` that need no instruction of their own."""


def isnan(x: float) -> bool:
    return x != x


def isinf(x: float) -> bool:
    return abs(x) == 1e999
