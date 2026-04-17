def sign(n: int) -> int:
    '''
    Returns the sign of *n*: -1 if negative, 0 if zero, 1 if positive.

    Examples
    >>> sign(-6)
    -1
    >>> sign(0)
    0
    >>> sign(10)
    1
    '''
    if n < 0:
        s = -1
    elif n == 0:
        s = 0
    else:
        s = 1
    return s


assert sign(-6) == -1
assert sign(0) == 0
assert sign(10) == 1
assert sign(-1) == -1
assert sign(1) == 1
