from typing import Callable


def logged(f: Callable[[int], int]) -> Callable[[int], int]:
    def wrapper(n: int) -> int:
        return f(n)

    return wrapper


@logged
def double(n: int) -> int:
    """
    >>> double(2)
    4
    """
    return 2 * n


class Box:
    def __init__(self) -> None:
        self._n = 0

    @property
    def n(self) -> int:
        return self._n

    @n.setter
    def n(self, value: int) -> None:
        """
        >>> b = Box()
        >>> b.n = 3
        >>> b.n
        3
        """
        self._n = value
