def em_ordem(lst: list[int]) -> bool:
    '''
    Produz True se os elementos de *lst* estão em ordem não decrescente, False
    caso contrário.

    Exemplos
    >>> em_ordem([])
    True
    >>> em_ordem([3])
    True
    >>> em_ordem([3, 4])
    True
    >>> em_ordem([4, 3])
    False
    >>> em_ordem([3, 3, 5, 6, 6])
    True
    >>> em_ordem([3, 3, 5, 4, 6])
    False
    '''
    if lst == []:
        ordem = True
    elif len(lst) == 1:
        ordem = True
    else:
        ordem = lst[0] <= lst[1] and em_ordem(lst[1:])
    return ordem

# Generated from doctests.
assert em_ordem([]) == True
assert em_ordem([3]) == True
assert em_ordem([3, 4]) == True
assert em_ordem([4, 3]) == False
assert em_ordem([3, 3, 5, 6, 6]) == True
assert em_ordem([3, 3, 5, 4, 6]) == False
