def insere_ordenado(lst: list[int], v: int):
    '''
    Insere *v* em *lst* de maneira que *lst* permaneça em ordem não
    decrescente.

    Requer que *lst* esteja em ordem não decrescente.

    Exemplos
    >>> lst = []
    >>> insere_ordenado(lst, 5)
    >>> lst
    [5]
    >>> insere_ordenado(lst, 3)
    >>> lst
    [3, 5]
    >>> insere_ordenado(lst, 4)
    >>> lst
    [3, 4, 5]
    >>> insere_ordenado(lst, 1)
    >>> lst
    [1, 3, 4, 5]
    >>> insere_ordenado(lst, 8)
    >>> lst
    [1, 3, 4, 5, 8]
    '''
    lst.append(v)
    i = len(lst) - 1
    while i > 0 and lst[i - 1] > lst[i]:
        # troca lst[i] <-> lst[i - 1]
        t = lst[i]
        lst[i] = lst[i - 1]
        lst[i - 1] = t
        i = i - 1

# Generated from doctests.
lst = []
insere_ordenado(lst, 5)
assert lst == [5]
insere_ordenado(lst, 3)
assert lst == [3, 5]
insere_ordenado(lst, 4)
assert lst == [3, 4, 5]
insere_ordenado(lst, 1)
assert lst == [1, 3, 4, 5]
insere_ordenado(lst, 8)
assert lst == [1, 3, 4, 5, 8]
