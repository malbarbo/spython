def recursive_max(lst: list[int]) -> int:
    '''
    Returns the maximum value in *lst* using recursion.
    Requires len(lst) > 0.

    Examples
    >>> recursive_max([3])
    3
    >>> recursive_max([3, 2])
    3
    >>> recursive_max([3, 2, 4])
    4
    >>> recursive_max([3, 2, 4, 1])
    4
    '''
    assert len(lst) > 0
    if len(lst) == 1:
        m = lst[0]
    else:
        rest = recursive_max(lst[1:])
        if lst[0] > rest:
            m = lst[0]
        else:
            m = rest
    return m


assert recursive_max([3]) == 3
assert recursive_max([3, 2]) == 3
assert recursive_max([3, 2, 4]) == 4
assert recursive_max([3, 2, 4, 1]) == 4
