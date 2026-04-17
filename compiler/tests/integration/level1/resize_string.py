def resize_string(s: str, n: int) -> str:
    '''
    Returns a new string with exactly *n* characters from *s*.
    If *s* has fewer than *n* characters, pads with spaces at the end.
    If *s* has more than *n* characters, truncates from the end.

    Requires n >= 0.

    Examples
    >>> resize_string('home', 7)
    'home   '
    >>> resize_string('computer', 4)
    'comp'
    >>> resize_string('python', 6)
    'python'
    '''
    assert n >= 0
    if len(s) < n:
        r = s + ' ' * (n - len(s))
    elif len(s) > n:
        r = s[:n]
    else:
        r = s
    return r


assert resize_string('home', 7) == 'home   '
assert resize_string('computer', 4) == 'comp'
assert resize_string('python', 6) == 'python'
assert resize_string('hi', 0) == ''
assert resize_string('', 3) == '   '
