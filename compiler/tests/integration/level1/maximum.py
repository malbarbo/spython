def maximum(a: int, b: int) -> int:
    '''
    Returns the maximum of *a* and *b*.

    Examples
    >>> maximum(10, 8)
    10
    >>> maximum(-2, -1)
    -1
    >>> maximum(6, 6)
    6
    '''
    if a > b:
        m = a
    else:
        m = b
    return m


def maximum3(a: int, b: int, c: int) -> int:
    '''
    Returns the maximum among *a*, *b*, and *c*.

    Examples
    >>> maximum3(20, 10, 12)
    20
    >>> maximum3(20, 12, 10)
    20
    >>> maximum3(20, 12, 12)
    20
    >>> maximum3(20, 20, 20)
    20
    >>> maximum3(5, 12, 3)
    12
    >>> maximum3(3, 12, 5)
    12
    >>> maximum3(5, 12, 5)
    12
    >>> maximum3(4, 8, 18)
    18
    >>> maximum3(8, 4, 18)
    18
    >>> maximum3(8, 8, 18)
    18
    '''
    return maximum(maximum(a, b), c)


assert maximum(10, 8) == 10
assert maximum(-2, -1) == -1
assert maximum(6, 6) == 6
assert maximum3(20, 10, 12) == 20
assert maximum3(20, 12, 10) == 20
assert maximum3(20, 12, 12) == 20
assert maximum3(20, 20, 20) == 20
assert maximum3(5, 12, 3) == 12
assert maximum3(3, 12, 5) == 12
assert maximum3(5, 12, 5) == 12
assert maximum3(4, 8, 18) == 18
assert maximum3(8, 4, 18) == 18
assert maximum3(8, 8, 18) == 18
