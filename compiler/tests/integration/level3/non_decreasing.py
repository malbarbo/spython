def is_non_decreasing(lst: list[int]) -> bool:
    '''
    Returns True if the elements of lst are in non-decreasing order,
    False otherwise.

    Examples
    >>> is_non_decreasing([])
    True
    >>> is_non_decreasing([4])
    True
    >>> is_non_decreasing([4, 6])
    True
    >>> is_non_decreasing([4, 2])
    False
    >>> is_non_decreasing([4, 6, 6])
    True
    >>> is_non_decreasing([4, 6, 5])
    False
    >>> is_non_decreasing([4, 6, 6, 7])
    True
    >>> is_non_decreasing([4, 3, 6, 7])
    False
    '''
    in_order = True
    for i in range(1, len(lst)):
        if lst[i - 1] > lst[i]:
            in_order = False
    return in_order


def is_non_decreasing2(lst: list[int]) -> bool:
    '''
    Returns True if the elements of lst are in non-decreasing order,
    False otherwise.

    Examples
    >>> is_non_decreasing2([])
    True
    >>> is_non_decreasing2([4])
    True
    >>> is_non_decreasing2([4, 6])
    True
    >>> is_non_decreasing2([4, 2])
    False
    >>> is_non_decreasing2([4, 6, 6])
    True
    >>> is_non_decreasing2([4, 6, 5])
    False
    >>> is_non_decreasing2([4, 6, 6, 7])
    True
    >>> is_non_decreasing2([4, 3, 6, 7])
    False
    '''
    in_order = True
    i = 1
    while i < len(lst) and in_order:
        if lst[i - 1] > lst[i]:
            in_order = False
        i = i + 1
    return in_order


assert is_non_decreasing([]) == True
assert is_non_decreasing([4]) == True
assert is_non_decreasing([4, 6]) == True
assert is_non_decreasing([4, 2]) == False
assert is_non_decreasing([4, 6, 6]) == True
assert is_non_decreasing([4, 6, 5]) == False
assert is_non_decreasing([4, 6, 6, 7]) == True
assert is_non_decreasing([4, 3, 6, 7]) == False
assert is_non_decreasing2([]) == True
assert is_non_decreasing2([4]) == True
assert is_non_decreasing2([4, 6]) == True
assert is_non_decreasing2([4, 2]) == False
assert is_non_decreasing2([4, 6, 6]) == True
assert is_non_decreasing2([4, 6, 5]) == False
assert is_non_decreasing2([4, 6, 6, 7]) == True
assert is_non_decreasing2([4, 3, 6, 7]) == False
