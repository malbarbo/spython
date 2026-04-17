def censor(phrase: str, n: int) -> str:
    '''
    Returns a string replacing the first *n* characters of *phrase* with *n* 'x's.

    Examples
    >>> censor('darn sandwich!', 4)
    'xxxx sandwich!'
    >>> censor('messed up!', 6)
    'xxxxxx up!'
    '''
    return 'x' * n + phrase[n:]


assert censor('darn sandwich!', 4) == 'xxxx sandwich!'
assert censor('messed up!', 6) == 'xxxxxx up!'
assert censor('hello', 0) == 'hello'
