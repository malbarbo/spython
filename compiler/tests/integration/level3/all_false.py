def all_false(lst: list[bool]) -> bool:
    '''
    Returns True if all elements of *lst* are False.

    Examples
    >>> all_false([])
    True
    >>> all_false([False])
    True
    >>> all_false([False, True, False])
    False
    '''
    result = True
    for b in lst:
        if b:
            result = False
    return result


assert all_false([]) == True
assert all_false([False]) == True
assert all_false([False, True, False]) == False
assert all_false([True]) == False
assert all_false([False, False]) == True
