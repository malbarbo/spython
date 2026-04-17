def is_prime(n: int) -> bool:
    '''
    Returns True if *n* is a prime number, that is, it has exactly two
    distinct divisors: 1 and itself. Returns False if *n* is not prime.

    Examples
    >>> is_prime(1)
    False
    >>> is_prime(2)
    True
    >>> is_prime(3)
    True
    >>> is_prime(5)
    True
    >>> is_prime(8)
    False
    >>> is_prime(11)
    True
    '''
    result = n != 1
    i = 2
    while i < n // 2 and result:
        if n % i == 0:
            result = False
        i = i + 1
    return result


assert is_prime(1) == False
assert is_prime(2) == True
assert is_prime(3) == True
assert is_prime(5) == True
assert is_prime(8) == False
assert is_prime(11) == True
