def indices_colunas_soma_zero(a: list[list[int]]) -> list[int]:
    '''
    Devolve uma lista com os índices das colunas cuja a soma é zero.

    Exemplo
    >>> indices_colunas_soma_zero([
    ... [6,  1, 2, 3],
    ... [2,  1, 1, -4],
    ... [7, -2, 4, 1]
    ... ])
    [1, 3]
    '''
    indices = []
    for j in range(len(a[0])):
        soma = 0
        for i in range(len(a)):
            soma = soma + a[i][j]
        if soma == 0:
            indices.append(j)
    return indices

# Generated from doctests.
assert indices_colunas_soma_zero([
[6,  1, 2, 3],
[2,  1, 1, -4],
[7, -2, 4, 1]
]) == [1, 3]
