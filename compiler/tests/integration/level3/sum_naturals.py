def sum_naturals(n: int) -> int:
    '''
    Sums all natural numbers less than or equal to *n*.

    Requires n >= 0.

    Examples
    >>> sum_naturals(0)
    0
    >>> sum_naturals(1)
    1
    >>> sum_naturals(2)
    3
    >>> sum_naturals(3)
    6
    >>> sum_naturals(4)
    10
    '''
    if n == 0:
        total = 0
    else:
        total = n + sum_naturals(n - 1)
    return total


assert sum_naturals(0) == 0
assert sum_naturals(1) == 1
assert sum_naturals(2) == 3
assert sum_naturals(3) == 6
assert sum_naturals(4) == 10
