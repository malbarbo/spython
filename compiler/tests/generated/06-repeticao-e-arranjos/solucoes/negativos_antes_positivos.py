def negativos_antes_positivos(lst: list[int]) -> list[int]:
    '''
    Cria uma nova lista com os elementos negativos de *lst*,
    seguidos dos elementos neutros e dos elementos positivos.
    Exemplos
    >>> negativos_antes_positivos([])
    []
    >>> negativos_antes_positivos([3, 0, -1, 0, 4])
    [-1, 0, 0, 3, 4]
    '''
    # Separa os negativos, neutros e positivos
    negativos = []
    neutros = []
    positivos = []
    for n in lst:
        if n < 0:
            negativos.append(n)
        elif n == 0:
            neutros.append(n)
        else:
            positivos.append(n)

    # Junta as listas
    return negativos + neutros + positivos

# Generated from doctests.
assert negativos_antes_positivos([]) == []
assert negativos_antes_positivos([3, 0, -1, 0, 4]) == [-1, 0, 0, 3, 4]
