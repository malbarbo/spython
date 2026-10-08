"""
>>> Point(1, 2).norm1()
3
"""

from dataclasses import dataclass


@dataclass
class Point:
    """
    >>> Point(0, 0)
    Point(x=0, y=0)
    """

    x: int
    y: int

    def norm1(self) -> int:
        """
        >>> Point(-1, 2).norm1()
        3
        """
        return abs(self.x) + abs(self.y)


def double(n: int) -> int:
    """
    >>> double(2)
    4
    """
    return 2 * n


twice = double
