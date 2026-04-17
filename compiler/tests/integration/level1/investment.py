def yield1(amount: float) -> float:
    '''
    Returns the yield on *amount* invested for one year.
    Rates: amount <= 2000 -> 10%, 2000 < amount <= 5000 -> 12%, amount > 5000 -> 13%.

    Examples
    >>> yield1(1000.0)
    100.0
    >>> yield1(2000.0)
    200.0
    >>> yield1(4000.0)
    480.0
    >>> yield1(5000.0)
    600.0
    >>> yield1(6000.0)
    780.0
    '''
    if amount <= 2000.0:
        r = amount * 0.1
    elif amount <= 5000.0:
        r = amount * 0.12
    else:
        r = amount * 0.13
    return r


def yield2(amount: float) -> float:
    '''
    Returns the yield on *amount* invested for two years.
    Rates: amount <= 2000 -> 10%, 2000 < amount <= 5000 -> 12%, amount > 5000 -> 13%.

    Examples
    >>> yield2(1000.0)
    110.0
    >>> yield2(2000.0)
    264.0
    >>> yield2(4000.0)
    537.6
    >>> yield2(5000.0)
    728.0
    >>> yield2(6000.0)
    881.4
    '''
    return yield1(amount + yield1(amount))


assert yield1(1000.0) == 100.0
assert yield1(2000.0) == 200.0
assert yield1(4000.0) == 480.0
assert yield1(5000.0) == 600.0
assert yield1(6000.0) == 780.0
assert yield2(1000.0) == 110.0
assert yield2(2000.0) == 264.0
assert yield2(4000.0) == 537.6
assert yield2(5000.0) == 728.0
assert yield2(6000.0) == 881.4
