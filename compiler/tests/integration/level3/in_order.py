def in_order(lst: list[int]) -> bool:
    '''
    Returns True if the elements of *lst* are in non-decreasing order,
    False otherwise.

    Examples
    >>> in_order([])
    True
    >>> in_order([3])
    True
    >>> in_order([3, 4])
    True
    >>> in_order([4, 3])
    False
    >>> in_order([3, 3, 5, 6, 6])
    True
    >>> in_order([3, 3, 5, 4, 6])
    False
    '''
    if lst == []:
        result = True
    elif len(lst) == 1:
        result = True
    else:
        result = lst[0] <= lst[1] and in_order(lst[1:])
    return result


assert in_order([]) == True
assert in_order([3]) == True
assert in_order([3, 4]) == True
assert in_order([4, 3]) == False
assert in_order([3, 3, 5, 6, 6]) == True
assert in_order([3, 3, 5, 4, 6]) == False
