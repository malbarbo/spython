def primeira_maiuscula(frase: str) -> str:
    '''
    Devolve uma nova string que é como *frase*, mas com apenas a primeira letra
    em maiúscula.

    Requer que *frase* começe com uma letra.

    Exemplos
    >>> primeira_maiuscula('joao venceu.')
    'Joao venceu.'
    >>> primeira_maiuscula('A Paula é um sucesso.')
    'A paula é um sucesso.'
    '''
    return frase[0].upper() + frase[1:].lower()

# Generated from doctests.
assert primeira_maiuscula('joao venceu.') == 'Joao venceu.'
assert primeira_maiuscula('A Paula é um sucesso.') == 'A paula é um sucesso.'
