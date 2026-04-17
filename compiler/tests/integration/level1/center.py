def center(s: str, n: int) -> str:
    '''
    Produces a string by padding *s* with spaces on both sides so that
    it reaches *n* characters total.

    If *s* already has *n* or more characters, returns *s* unchanged.

    The number of spaces added at the start is equal to or one more than
    the number added at the end.

    Examples
    >>> center('home', 3)
    'home'
    >>> center('', 0)
    ''
    >>> center('home', 10)
    '   home   '
    >>> center('home', 9)
    '   home  '
    >>> center('Python', 10)
    '  Python  '
    >>> center('Python', 9)
    '  Python '
    '''
    if len(s) >= n:
        r = s
    else:
        missing = n - len(s)
        end = missing // 2
        start = missing - end
        r = ' ' * start + s + ' ' * end
    return r


assert center('home', 3) == 'home'
assert center('', 0) == ''
assert center('home', 10) == '   home   '
assert center('home', 9) == '   home  '
assert center('Python', 10) == '  Python  '
assert center('Python', 9) == '  Python '
