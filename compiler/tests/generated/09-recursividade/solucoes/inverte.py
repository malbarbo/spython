def inverte(lst: list[int], inicio: int, fim: int):
    '''
    Inverte a ordem dos elementos de *lst[inicio:fim]*.

    Requer que 0 <= inicio <= fim <= len(lst).

    Exemplos
    >>> lst = [4, 1, 5, 7, 3, 9]
    >>> inverte(lst, 0, len(lst))
    >>> lst
    [9, 3, 7, 5, 1, 4]
    >>> inverte(lst, 2, 5)
    >>> lst
    [9, 3, 1, 5, 7, 4]
    '''
    assert 0 <= inicio <= fim <= len(lst)
    if inicio < fim - 1:
        t = lst[inicio]
        lst[inicio] = lst[fim - 1]
        lst[fim - 1] = t
        inverte(lst, inicio + 1, fim - 1)

# Generated from doctests.
lst = [4, 1, 5, 7, 3, 9]
inverte(lst, 0, len(lst))
assert lst == [9, 3, 7, 5, 1, 4]
inverte(lst, 2, 5)
assert lst == [9, 3, 1, 5, 7, 4]
