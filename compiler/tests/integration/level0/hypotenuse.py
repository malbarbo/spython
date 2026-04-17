def square(a: float) -> float:
    '''
    Calculates the square of *a*.

    Examples
    >>> square(4.0)
    16.0
    '''
    return a * a


def sqrt(a: float) -> float:
    '''
    Calculates the square root of *a*.
    Requires *a* to be non-negative.

    Examples
    >>> sqrt(4.0)
    2.0
    '''
    return a ** 0.5


def hypotenuse(a: float, b: float) -> float:
    '''
    Calculates the hypotenuse of a right triangle with legs *a* and *b*.
    Requires *a* and *b* to be positive.

    Examples
    >>> hypotenuse(3.0, 4.0)
    5.0
    '''
    a2 = square(a)
    b2 = square(b)
    return sqrt(a2 + b2)


assert square(4.0) == 16.0
assert sqrt(4.0) == 2.0
assert hypotenuse(3.0, 4.0) == 5.0
