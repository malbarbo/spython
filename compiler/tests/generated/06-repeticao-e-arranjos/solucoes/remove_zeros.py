def remove_zeros(lst: list[int]) -> list[int]:
    '''
    Produz uma nova lista removendo todos os valores zeros de *lst*.

    Exemplos
    >>> remove_zeros([])
    []
    >>> remove_zeros([4, 1, 0, 3, 0])
    [4, 1, 3]
    '''
    sem_zeros = []
    for n in lst:
        if n != 0:
            sem_zeros.append(n)
    return sem_zeros

# Generated from doctests.
assert remove_zeros([]) == []
assert remove_zeros([4, 1, 0, 3, 0]) == [4, 1, 3]
