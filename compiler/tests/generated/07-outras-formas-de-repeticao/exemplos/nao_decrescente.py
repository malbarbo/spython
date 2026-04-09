def ordem_nao_decrescente(lst: list[int]) -> bool:
    '''
    Produz True se os elementos de lst estão em ordem não decrescente,
    False caso contrário.

    Exemplos
    >>> ordem_nao_decrescente([])
    True
    >>> ordem_nao_decrescente([4])
    True
    >>> ordem_nao_decrescente([4, 6])
    True
    >>> ordem_nao_decrescente([4, 2])
    False
    >>> ordem_nao_decrescente([4, 6, 6])
    True
    >>> ordem_nao_decrescente([4, 6, 5])
    False
    >>> ordem_nao_decrescente([4, 6, 6, 7])
    True
    >>> ordem_nao_decrescente([4, 3, 6, 7])
    False
    '''
    em_ordem = True
    for i in range(1, len(lst)):
        if lst[i - 1] > lst[i]:
            em_ordem = False
    return em_ordem

def ordem_nao_decrescente2(lst: list[int]) -> bool:
    '''
    Produz True se os elementos de lst estão em ordem não decrescente,
    False caso contrário.

    Exemplos
    >>> ordem_nao_decrescente2([])
    True
    >>> ordem_nao_decrescente2([4])
    True
    >>> ordem_nao_decrescente2([4, 6])
    True
    >>> ordem_nao_decrescente2([4, 2])
    False
    >>> ordem_nao_decrescente2([4, 6, 6])
    True
    >>> ordem_nao_decrescente2([4, 6, 5])
    False
    >>> ordem_nao_decrescente2([4, 6, 6, 7])
    True
    >>> ordem_nao_decrescente2([4, 3, 6, 7])
    False
    '''
    em_ordem = True
    i = 1
    while i < len(lst) and em_ordem:
        if lst[i - 1] > lst[i]:
            em_ordem = False
        i = i + 1
    return em_ordem

# Generated from doctests.
assert ordem_nao_decrescente([]) == True
assert ordem_nao_decrescente([4]) == True
assert ordem_nao_decrescente([4, 6]) == True
assert ordem_nao_decrescente([4, 2]) == False
assert ordem_nao_decrescente([4, 6, 6]) == True
assert ordem_nao_decrescente([4, 6, 5]) == False
assert ordem_nao_decrescente([4, 6, 6, 7]) == True
assert ordem_nao_decrescente([4, 3, 6, 7]) == False
assert ordem_nao_decrescente2([]) == True
assert ordem_nao_decrescente2([4]) == True
assert ordem_nao_decrescente2([4, 6]) == True
assert ordem_nao_decrescente2([4, 2]) == False
assert ordem_nao_decrescente2([4, 6, 6]) == True
assert ordem_nao_decrescente2([4, 6, 5]) == False
assert ordem_nao_decrescente2([4, 6, 6, 7]) == True
assert ordem_nao_decrescente2([4, 3, 6, 7]) == False
