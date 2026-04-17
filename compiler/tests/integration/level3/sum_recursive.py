def sum_list(lst: list[int]) -> int:
    '''
    Sums the elements of *lst*.

    Examples
    >>> sum_list([])
    0
    >>> sum_list([6])
    6
    >>> sum_list([3, 6])
    9
    >>> sum_list([7, 3, 6])
    16
    '''
    if lst == []:
        s = 0
    else:
        s = lst[0] + sum_list(lst[1:])
    return s


def sum_from(lst: list[int], i: int) -> int:
    '''
    Sums the elements of *lst* starting at index *i*, that is,
    sums the elements of lst[i:].
    Requires 0 <= i <= len(lst).

    Examples
    >>> sum_from([7, 3, 6], 0)
    16
    >>> sum_from([7, 3, 6], 1)
    9
    >>> sum_from([7, 3, 6], 2)
    6
    >>> sum_from([7, 3, 6], 3)
    0
    '''
    if i >= len(lst):
        s = 0
    else:
        s = lst[i] + sum_from(lst, i + 1)
    return s


def sum_first(lst: list[int], i: int) -> int:
    '''
    Sums the first *i* elements of *lst*, that is, sums lst[:i].
    Requires 0 <= i <= len(lst).

    Examples
    >>> sum_first([7, 3, 6], 0)
    0
    >>> sum_first([7, 3, 6], 1)
    7
    >>> sum_first([7, 3, 6], 2)
    10
    >>> sum_first([7, 3, 6], 3)
    16
    '''
    if i <= 0:
        s = 0
    else:
        s = lst[i - 1] + sum_first(lst, i - 1)
    return s


assert sum_list([]) == 0
assert sum_list([6]) == 6
assert sum_list([3, 6]) == 9
assert sum_list([7, 3, 6]) == 16
assert sum_from([7, 3, 6], 0) == 16
assert sum_from([7, 3, 6], 1) == 9
assert sum_from([7, 3, 6], 2) == 6
assert sum_from([7, 3, 6], 3) == 0
assert sum_first([7, 3, 6], 0) == 0
assert sum_first([7, 3, 6], 1) == 7
assert sum_first([7, 3, 6], 2) == 10
assert sum_first([7, 3, 6], 3) == 16
