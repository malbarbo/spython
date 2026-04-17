def even(n: int) -> bool:
    '''
    Returns True if *n* is even (multiple of 2).
    Requires n >= 0.

    Examples
    >>> even(0)
    True
    >>> even(1)
    False
    >>> even(2)
    True
    >>> even(3)
    False
    >>> even(4)
    True
    '''
    assert n >= 0
    if n == 0:
        p = True
    else:
        p = odd(n - 1)
    return p


def odd(n: int) -> bool:
    '''
    Returns True if *n* is odd (not a multiple of 2).
    Requires n >= 0.

    Examples
    >>> odd(0)
    False
    >>> odd(1)
    True
    >>> odd(2)
    False
    >>> odd(3)
    True
    >>> odd(4)
    False
    '''
    assert n >= 0
    if n == 0:
        p = False
    else:
        p = even(n - 1)
    return p


assert even(0) == True
assert even(1) == False
assert even(2) == True
assert even(3) == False
assert even(4) == True
assert odd(0) == False
assert odd(1) == True
assert odd(2) == False
assert odd(3) == True
assert odd(4) == False
