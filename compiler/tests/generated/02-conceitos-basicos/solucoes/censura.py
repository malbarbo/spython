def censura(frase: str, n: int) -> str:
    '''
    Produz uma string trocando as as primeiras *n* letras de *frase* por *n* 'x'.

    Exemplos
    >>> censura('droga de lanche!', 5)
    'xxxxx de lanche!'
    >>> censura('ferrou geral!', 6)
    'xxxxxx geral!'
    '''
    return 'x' * n + frase[n:]

# Generated from doctests.
assert censura('droga de lanche!', 5) == 'xxxxx de lanche!'
assert censura('ferrou geral!', 6) == 'xxxxxx geral!'
