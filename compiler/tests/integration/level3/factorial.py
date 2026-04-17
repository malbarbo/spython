def factorial(n: int) -> int:
    '''
    Calculates the product of all natural numbers from 1 to n,
    that is, 1 * ... * (n - 1) * n.

    Examples
    >>> factorial(0)
    1
    >>> factorial(1)
    1
    >>> factorial(2)
    2
    >>> factorial(3)
    6
    >>> factorial(4)
    24
    '''
    result = 1
    for i in range(2, n + 1):
        result = result * i
    return result


assert factorial(0) == 1
assert factorial(1) == 1
assert factorial(2) == 2
assert factorial(3) == 6
assert factorial(4) == 24
