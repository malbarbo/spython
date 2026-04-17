def clamp(n: int, minimum: int, maximum: int) -> int:
    '''
    Clamps *n* to the range [*minimum*, *maximum*].

    Examples
    >>> clamp(1, 2, 10)
    2
    >>> clamp(2, 2, 10)
    2
    >>> clamp(5, 2, 10)
    5
    >>> clamp(10, 2, 10)
    10
    >>> clamp(14, 2, 10)
    10
    '''
    if n < minimum:
        r = minimum
    elif maximum < n:
        r = maximum
    else:
        r = n
    return r


assert clamp(1, 2, 10) == 2
assert clamp(2, 2, 10) == 2
assert clamp(5, 2, 10) == 5
assert clamp(10, 2, 10) == 10
assert clamp(14, 2, 10) == 10
