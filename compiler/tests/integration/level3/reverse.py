def reversed_list(lst: list[int]) -> list[int]:
    '''
    Creates a new list with the elements of *lst* in reverse order.

    Examples
    >>> reversed_list([])
    []
    >>> reversed_list([1])
    [1]
    >>> reversed_list([6, 1])
    [1, 6]
    >>> reversed_list([5, 1, 4])
    [4, 1, 5]
    '''
    r = []
    for i in range(len(lst)):
        r.append(lst[len(lst) - i - 1])
    return r


def reverse_in_place(lst: list[int]):
    '''
    Reverses the elements of *lst* in place.

    Examples
    >>> x = [5, 4, 1]
    >>> reverse_in_place(x)
    >>> x
    [1, 4, 5]
    >>> x = [5, 4, 1, 6, 8]
    >>> reverse_in_place(x)
    >>> x
    [8, 6, 1, 4, 5]
    '''
    for i in range(len(lst) // 2):
        t = lst[i]
        lst[i] = lst[len(lst) - i - 1]
        lst[len(lst) - i - 1] = t


assert reversed_list([]) == []
assert reversed_list([1]) == [1]
assert reversed_list([6, 1]) == [1, 6]
assert reversed_list([5, 1, 4]) == [4, 1, 5]
x: list[int] = [5, 4, 1]
reverse_in_place(x)
assert x == [1, 4, 5]
x = [5, 4, 1, 6, 8]
reverse_in_place(x)
assert x == [8, 6, 1, 4, 5]
