def doubled_list(lst: list[int]) -> bool:
    '''
    Returns True if *lst* can be split into two equal halves.

    Examples
    >>> doubled_list([])
    True
    >>> doubled_list([3])
    False
    >>> doubled_list([3, 3])
    True
    >>> doubled_list([3, 2])
    False
    >>> doubled_list([2, 6, 1, 2, 6, 1])
    True
    >>> doubled_list([2, 6, 1, 2, 6, 1, 4])
    False
    '''
    doubled = len(lst) % 2 == 0
    mid = len(lst) // 2
    i = 0
    while i < mid and doubled:
        if lst[i] != lst[mid + i]:
            doubled = False
        i = i + 1
    return doubled


assert doubled_list([]) == True
assert doubled_list([3]) == False
assert doubled_list([3, 3]) == True
assert doubled_list([3, 2]) == False
assert doubled_list([2, 6, 1, 2, 6, 1]) == True
assert doubled_list([2, 6, 1, 2, 6, 1, 4]) == False
