def frequency(v: int, lst: list[int]) -> int:
    '''
    Counts how many times *v* appears in *lst*.

    Examples
    >>> frequency(1, [])
    0
    >>> frequency(1, [7])
    0
    >>> frequency(1, [1, 7, 1])
    2
    >>> frequency(4, [4, 1, 7, 4, 4])
    3
    '''
    if lst == []:
        count = 0
    else:
        if v == lst[0]:
            count = 1 + frequency(v, lst[1:])
        else:
            count = frequency(v, lst[1:])
    return count


assert frequency(1, []) == 0
assert frequency(1, [7]) == 0
assert frequency(1, [1, 7, 1]) == 2
assert frequency(4, [4, 1, 7, 4, 4]) == 3
