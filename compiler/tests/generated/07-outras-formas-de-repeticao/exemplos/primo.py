def primo(n: int) -> bool:
    '''
    Produz True se *n* é um número primo, isto é, tem exatamente dois divisores
    distintos, 1 e ele mesmo. Produz False se *n* não é primo.

    Exemplos
    >>> primo(1) # 1
    False
    >>> primo(2) # 1 2
    True
    >>> primo(3) # 1 3
    True
    >>> primo(5) # 1 5
    True
    >>> primo(8) # 1 2 4 8
    False
    >>> primo(11) # 1 11
    True
    '''
    eh_primo = n != 1
    i = 2
    while i < n // 2 and eh_primo:
        if n % i == 0:
            eh_primo = False
        i = i + 1
    return eh_primo

# Generated from doctests.
assert primo(1) == False
assert primo(2) == True
assert primo(3) == True
assert primo(5) == True
assert primo(8) == False
assert primo(11) == True
