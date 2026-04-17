def find_starts_with_a(lst: list[str]) -> list[str]:
    '''
    Finds the elements of *lst* that start with 'A'.

    Examples
    >>> find_starts_with_a([])
    []
    >>> find_starts_with_a(['Ali'])
    ['Ali']
    >>> find_starts_with_a(['Ali', 'ala'])
    ['Ali']
    >>> find_starts_with_a(['Ali', 'ala', 'Alto'])
    ['Ali', 'Alto']
    >>> find_starts_with_a(['Ali', 'ala', 'Alto', ''])
    ['Ali', 'Alto']
    '''
    starts_with_a = []
    for s in lst:
        if s != '' and s[0] == 'A':
            starts_with_a.append(s)
    return starts_with_a


assert find_starts_with_a([]) == []
assert find_starts_with_a(['Ali']) == ['Ali']
assert find_starts_with_a(['Ali', 'ala']) == ['Ali']
assert find_starts_with_a(['Ali', 'ala', 'Alto']) == ['Ali', 'Alto']
assert find_starts_with_a(['Ali', 'ala', 'Alto', '']) == ['Ali', 'Alto']
