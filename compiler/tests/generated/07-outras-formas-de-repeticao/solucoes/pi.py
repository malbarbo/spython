def pi(n: int) -> float:
    '''
    Cacula pi usando os primeiros *n* termos da série
    pi = 4 * (1/1 - 1/3 + 1/5 - 1/7 + 1/9 - ...)
    Requer que n > 0

    Exemplos
    >>> pi(1)
    4.0
    >>> pi(2)
    2.666666666666667
    >>> pi(3)
    3.466666666666667
    >>> pi(4)
    2.8952380952380956
    >>> pi(5)
    3.3396825396825403
    '''
    num = 1.0
    denum = 1.0
    pi_4 = num / denum
    for k in range(1, n):
        num = num * -1.0
        denum = denum + 2.0
        pi_4 = pi_4 + num / denum
    return pi_4 * 4.0

# Generated from doctests.
assert pi(1) == 4.0
assert pi(2) == 2.666666666666667
assert pi(3) == 3.466666666666667
assert pi(4) == 2.8952380952380956
assert pi(5) == 3.3396825396825403
