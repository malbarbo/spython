
def avg_length(lst: list[str]) -> float:
    '''
    Calculates the average length of the strings in *lst*.
    Requires *lst* to be non-empty.

    Examples
    >>> avg_length(['home'])
    4.0
    >>> avg_length(['home', 'it'])
    3.0
    >>> avg_length(['home', 'it', ''])
    2.0
    >>> avg_length(['home', 'it', '', 'once'])
    2.5
    '''
    count = 0
    avg = 0.0
    for s in lst:
        avg = (count * avg + len(s)) / (count + 1)
        count = count + 1
    return avg


def avg_length2(lst: list[str]) -> float:
    '''
    Calculates the average length of the strings in *lst*.
    Requires *lst* to be non-empty.

    Examples
    >>> avg_length2(['home'])
    4.0
    >>> avg_length2(['home', 'it'])
    3.0
    >>> avg_length2(['home', 'it', ''])
    2.0
    >>> avg_length2(['home', 'it', '', 'once'])
    2.5
    '''
    assert len(lst) != 0

    total = 0
    for s in lst:
        total = total + len(s)

    return total / len(lst)


assert avg_length(['home']) == 4.0
assert avg_length(['home', 'it']) == 3.0
assert avg_length(['home', 'it', '']) == 2.0
assert avg_length(['home', 'it', '', 'once']) == 2.5
assert avg_length2(['home']) == 4.0
assert avg_length2(['home', 'it']) == 3.0
assert avg_length2(['home', 'it', '']) == 2.0
assert avg_length2(['home', 'it', '', 'once']) == 2.5
