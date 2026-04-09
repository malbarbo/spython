def transposta(a: list[list[int]]) -> list[list[int]]:
    '''
    Cria a matriz transposta de *m*.

    Requer que *m* seja regular.

    Exemplos
    >>> transposta([[4, 5, 1], [7, 8, 9]])
    [[4, 7], [5, 8], [1, 9]]
    >>> transposta([[4, 1], [7, 8], [2, 6], [5, 3]])
    [[4, 7, 2, 5], [1, 8, 6, 3]]
    '''
    t = []
    for j in range(len(a[0])):
        coluna = []
        for i in range(len(a)):
            coluna.append(a[i][j])
        t.append(coluna)
    return t

# Generated from doctests.
assert transposta([[4, 5, 1], [7, 8, 9]]) == [[4, 7], [5, 8], [1, 9]]
assert transposta([[4, 1], [7, 8], [2, 6], [5, 3]]) == [[4, 7, 2, 5], [1, 8, 6, 3]]
