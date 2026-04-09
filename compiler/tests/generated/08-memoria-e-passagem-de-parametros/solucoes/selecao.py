def ordena_selecao(lst: list[int]):
    '''
    Ordena os valores de *lst* em ordem não decrescente.

    Exemplos
    >>> lst = [8, 5, 4, 1, 2]
    >>> ordena_selecao(lst)
    >>> lst
    [1, 2, 4, 5, 8]
    '''
    # A sublista lst[:i] está ordenada
    for i in range(len(lst) - 1):
        # Índice do elemento mínimo de lst[i:]
        jmin = i
        for j in range(i + 1, len(lst)):
            if lst[j] < lst[jmin]:
                jmin = j

        # Troca lst[i] <-> lst[jmin]
        t = lst[i]
        lst[i] = lst[jmin]
        lst[jmin] = t

# Generated from doctests.
lst = [8, 5, 4, 1, 2]
ordena_selecao(lst)
assert lst == [1, 2, 4, 5, 8]
