def zero_last_digits(n: int) -> int:
    '''
    Returns *n* with the last two digits set to zero.

    Examples
    >>> zero_last_digits(19)
    0
    >>> zero_last_digits(341)
    300
    >>> zero_last_digits(5251)
    5200
    '''
    return n // 100 * 100


assert zero_last_digits(19) == 0
assert zero_last_digits(341) == 300
assert zero_last_digits(5251) == 5200
assert zero_last_digits(100) == 100
