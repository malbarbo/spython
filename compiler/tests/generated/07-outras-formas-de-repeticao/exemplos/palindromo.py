def palindromo(lst: list[int]) -> bool:
    '''
    Produz True se *lst* é palíndromo, isto é, tem os mesmos elementos quando
    vistos da direira para esquerda e da esquerda para direita. Produz False
    caso contrário.

    Exemplos
    >>> palindromo([])
    True
    >>> palindromo([4])
    True
    >>> palindromo([1, 1])
    True
    >>> palindromo([1, 2])
    False
    >>> palindromo([1, 2, 1])
    True
    >>> palindromo([1, 5, 5, 1])
    True
    >>> palindromo([1, 5, 1, 5])
    False
    '''
    eh_palindromo = True
    i = 0
    j = len(lst) - 1
    while i < j and eh_palindromo:
        if lst[i] != lst[j]:
            eh_palindromo = False
        i = i + 1
        j = j - 1
    return eh_palindromo

# Generated from doctests.
assert palindromo([]) == True
assert palindromo([4]) == True
assert palindromo([1, 1]) == True
assert palindromo([1, 2]) == False
assert palindromo([1, 2, 1]) == True
assert palindromo([1, 5, 5, 1]) == True
assert palindromo([1, 5, 1, 5]) == False
