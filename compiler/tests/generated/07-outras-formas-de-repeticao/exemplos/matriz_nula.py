def cria_matriz_nula(m: int, n: int) -> list[list[int]]:
    '''
    Cria uma matriz nula com *m* linhas e *n* colunas.

    Requer que m > 0 e n > 0.

    Exemplos
    >>> cria_matriz_nula(2, 3)
    [[0, 0, 0], [0, 0, 0]]
    '''
    a = []
    for i in range(m):
        linha = []
        for j in range(n):
            linha.append(0)
        a.append(linha)
    return a

# Generated from doctests.
assert cria_matriz_nula(2, 3) == [[0, 0, 0], [0, 0, 0]]
