def power(base: float, exp: int) -> float:
    '''
    Calculates *base* raised to *exp*.
    Requires base != 0 and exp >= 0.

    Examples
    >>> power(2.0, 0)
    1.0
    >>> power(2.0, 1)
    2.0
    >>> power(2.0, 2)
    4.0
    >>> power(2.0, 3)
    8.0
    >>> power(3.0, 3)
    27.0
    >>> power(3.0, 4)
    81.0
    '''
    if exp == 0:
        result = 1.0
    else:
        result = base * power(base, exp - 1)
    return result


assert power(2.0, 0) == 1.0
assert power(2.0, 1) == 2.0
assert power(2.0, 2) == 4.0
assert power(2.0, 3) == 8.0
assert power(3.0, 3) == 27.0
assert power(3.0, 4) == 81.0
