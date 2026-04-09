def remove_indice(lst: list[int], i: int):
    '''
    Remove o elemento do índice *i* de *lst* movendo
    os elementos das posições i + 1, i + 2, ..., len(lst)
    para as posições i, i + 1, ..., len(lst) - 1.

    Requer que 0 <= i < len(lst).

    Exemplos
    >>> lst = [7, 1, 8, 9]
    >>> remove_indice(lst, 2)
    >>> lst
    [7, 1, 9]
    >>> remove_indice(lst, 0)
    >>> lst
    [1, 9]
    >>> remove_indice(lst, 1)
    >>> lst
    [1]
    >>> remove_indice(lst, 0)
    >>> lst
    []
    '''
    assert 0 <= i < len(lst)
    while i < len(lst) - 1:
        lst[i] = lst[i + 1]
        i = i + 1
    lst.pop()

# Generated from doctests.
lst = [7, 1, 8, 9]
remove_indice(lst, 2)
assert lst == [7, 1, 9]
remove_indice(lst, 0)
assert lst == [1, 9]
remove_indice(lst, 1)
assert lst == [1]
remove_indice(lst, 0)
assert lst == []
