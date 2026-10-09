from math import *

total: int = 1


def current() -> int:
    """
    >>> total
    1
    >>> total = "a"
    >>> total + "b"
    'ab'
    >>> current()
    1
    """
    return total


def next_total() -> int:
    """
    >>> total + 1
    2
    >>> round(pi)
    3
    >>> pi = 3
    >>> pi
    3
    """
    return total + 1
