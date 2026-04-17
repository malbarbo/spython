def insert_sorted(lst: list[int], v: int):
    '''
    Inserts *v* into *lst* so that *lst* remains in non-decreasing order.

    Requires *lst* to be in non-decreasing order.

    Examples
    >>> lst = []
    >>> insert_sorted(lst, 5)
    >>> lst
    [5]
    >>> insert_sorted(lst, 3)
    >>> lst
    [3, 5]
    >>> insert_sorted(lst, 4)
    >>> lst
    [3, 4, 5]
    '''
    lst.append(v)
    i = len(lst) - 1
    while i > 0 and lst[i - 1] > lst[i]:
        t = lst[i]
        lst[i] = lst[i - 1]
        lst[i - 1] = t
        i = i - 1


lst: list[int] = []
insert_sorted(lst, 5)
assert lst == [5]
insert_sorted(lst, 3)
assert lst == [3, 5]
insert_sorted(lst, 4)
assert lst == [3, 4, 5]
insert_sorted(lst, 1)
assert lst == [1, 3, 4, 5]
insert_sorted(lst, 8)
assert lst == [1, 3, 4, 5, 8]
