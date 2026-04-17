def units(n: int) -> int:
    '''
    Returns the units digit of *n*.

    Examples
    >>> units(152)
    2
    '''
    return n % 10


def tens(n: int) -> int:
    '''
    Returns the tens digit of *n*.

    Examples
    >>> tens(152)
    5
    '''
    return n // 10 % 10


def hundreds(n: int) -> int:
    '''
    Returns the hundreds digit of *n*.

    Examples
    >>> hundreds(152)
    1
    '''
    return n // 100 % 10


assert units(152) == 2
assert tens(152) == 5
assert hundreds(152) == 1
assert units(0) == 0
assert tens(10) == 1
assert hundreds(300) == 3
