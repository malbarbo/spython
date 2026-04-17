def factorial(n: int) -> int:
    '''
    Returns the factorial of *n*.
    Requires n >= 0.

    Examples
    >>> factorial(0)
    1
    >>> factorial(1)
    1
    >>> factorial(4)
    24
    '''
    assert n >= 0
    if n == 0:
        f = 1
    else:
        f = n * factorial(n - 1)
    return f


assert factorial(0) == 1
assert factorial(1) == 1
assert factorial(2) == 2
assert factorial(3) == 6
assert factorial(4) == 24
