def rotate_right(s: str, n: int) -> str:
    '''
    Returns a string with the last *n* characters of *s* moved to the beginning.
    If *n* is greater than the length of *s* or negative, rotates (n % len(s)) characters.

    Examples
    >>> rotate_right('marcelio', 5)
    'celiomar'
    >>> rotate_right('abc', 0)
    'abc'
    >>> rotate_right('abc', 1)
    'cab'
    >>> rotate_right('abc', 2)
    'bca'
    >>> rotate_right('abc', 3)
    'abc'
    '''
    div = len(s) - n % len(s)
    return s[div:] + s[:div]


assert rotate_right('marcelio', 5) == 'celiomar'
assert rotate_right('abc', 0) == 'abc'
assert rotate_right('abc', 1) == 'cab'
assert rotate_right('abc', 2) == 'bca'
assert rotate_right('abc', 3) == 'abc'
