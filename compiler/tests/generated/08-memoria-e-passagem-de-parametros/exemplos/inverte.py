def inverte(lst: list[int]) -> list[int]:
    '''
    Cria uma nova lista invertendo os elementos de *lst*, isto é, o último
    aparece como primeiro, o penúltimo com segundo, e assim por diante.

    Exemlos
    >>> inverte([])
    []
    >>> inverte([1])
    [1]
    >>> inverte([6, 1])
    [1, 6]
    >>> inverte([5, 1, 4])
    [4, 1, 5]
    '''
    r = []
    for i in range(len(lst)):
        r.append(lst[len(lst) - i - 1])
    # ou
    # i = len(lst) - 1
    # while i >= 0:
    #     r.append(lst[i])
    #     i = i - 1
    return r

def invertem(lst: list[int]):
    '''
    Modifica *lst* invertendo a ordem dos elementos, isto é, colocando o último
    elemento na primeiro posição, o penúltimo elemento na segunda posição, e
    assim por diante.

    Exemplos
    >>> x = [5, 4, 1]
    >>> invertem(x)
    >>> x
    [1, 4, 5]
    >>> x = [5, 4, 1, 6, 8]
    >>> invertem(x)
    >>> x
    [8, 6, 1, 4, 5]
    '''
    for i in range(len(lst) // 2):
        t = lst[i]
        lst[i] = lst[len(lst) - i - 1]
        lst[len(lst) - i - 1] = t

# Generated from doctests.
assert inverte([]) == []
assert inverte([1]) == [1]
assert inverte([6, 1]) == [1, 6]
assert inverte([5, 1, 4]) == [4, 1, 5]
x = [5, 4, 1]
invertem(x)
assert x == [1, 4, 5]
x = [5, 4, 1, 6, 8]
invertem(x)
assert x == [8, 6, 1, 4, 5]
