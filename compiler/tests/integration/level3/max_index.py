def max_index(lst: list[int]) -> int:
    '''
    Finds the index of the first occurrence of the maximum value in *lst*.
    Requires *lst* to be non-empty.

    Examples
    >>> max_index([5])
    0
    >>> max_index([5, 6])
    1
    >>> max_index([5, 6, 6])
    1
    >>> max_index([5, 6, 6, 8])
    3
    '''
    assert len(lst) != 0
    imax = 0
    for i in range(1, len(lst)):
        if lst[i] > lst[imax]:
            imax = i
    return imax


assert max_index([5]) == 0
assert max_index([5, 6]) == 1
assert max_index([5, 6, 6]) == 1
assert max_index([5, 6, 6, 8]) == 3
