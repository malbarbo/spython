def pi_series(n: int) -> float:
    '''
    Approximates pi using the first *n* terms of the Leibniz series:
    pi = 4 * (1/1 - 1/3 + 1/5 - 1/7 + ...)
    Requires n > 0.

    Examples
    >>> pi_series(1)
    4.0
    >>> pi_series(2)
    2.666666666666667
    '''
    num = 1.0
    denom = 1.0
    result = num / denom
    k = 1
    while k < n:
        num = num * -1.0
        denom = denom + 2.0
        result = result + num / denom
        k = k + 1
    return result * 4.0


assert pi_series(1) == 4.0
assert pi_series(2) == 2.666666666666667
assert pi_series(3) == 3.466666666666667
