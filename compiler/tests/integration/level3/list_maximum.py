def list_maximum(lst: list[int]) -> int:
    '''
    Finds the maximum value in *lst*.
    Requires lst to be non-empty.

    Examples
    >>> list_maximum([2])
    2
    >>> list_maximum([2, 4])
    4
    >>> list_maximum([2, 4, 3])
    4
    >>> list_maximum([2, 4, 3, 7])
    7
    '''
    assert len(lst) != 0
    max_val = lst[0]
    for n in lst:
        if n > max_val:
            max_val = n
    return max_val


assert list_maximum([2]) == 2
assert list_maximum([2, 4]) == 4
assert list_maximum([2, 4, 3]) == 4
assert list_maximum([2, 4, 3, 7]) == 7
