def is_regular(a: list[list[int]]) -> bool:
    '''
    Returns True if *a* is a regular matrix, that is, all rows have the
    same number of elements.

    Examples
    >>> is_regular([])
    True
    >>> is_regular([[2]])
    True
    >>> is_regular([[2], [4]])
    True
    >>> is_regular([[2], [4, 1]])
    False
    >>> is_regular([[2, 2], [4]])
    False
    >>> is_regular([[2, 1, 6], [4, 0, 1]])
    True
    >>> is_regular([[2, 1], [4, 0, 1]])
    False
    >>> is_regular([[2, 1], [4]])
    False
    >>> is_regular([[2], [4], [7]])
    True
    >>> is_regular([[2], [4], [7, 2]])
    False
    '''
    regular = True
    i = 1
    while i < len(a) and regular:
        if len(a[0]) != len(a[i]):
            regular = False
        i = i + 1
    return regular


assert is_regular([]) == True
assert is_regular([[2]]) == True
assert is_regular([[2], [4]]) == True
assert is_regular([[2], [4, 1]]) == False
assert is_regular([[2, 2], [4]]) == False
assert is_regular([[2, 1, 6], [4, 0, 1]]) == True
assert is_regular([[2, 1], [4, 0, 1]]) == False
assert is_regular([[2, 1], [4]]) == False
assert is_regular([[2], [4], [7]]) == True
assert is_regular([[2], [4], [7, 2]]) == False
