def sem_espacos_inicio_fim(texto: str) -> bool:
    '''
    Produz True se *texto* não começa e nem termina com espaços.
    Produz False, caso contrário.

    Exemplos
    >>> sem_espacos_inicio_fim('')
    True
    >>> sem_espacos_inicio_fim('Começa com espaço? Não.')
    True
    >>> sem_espacos_inicio_fim(' Começa com espaço? Sim!')
    False
    >>> sem_espacos_inicio_fim('Termina com espaço? Sim! ')
    False
    >>> sem_espacos_inicio_fim(' no início e fim ')
    False
    '''
    return texto == '' or (texto[0] != ' ' and texto[-1] != ' ')

# Generated from doctests.
assert sem_espacos_inicio_fim('') == True
assert sem_espacos_inicio_fim('Começa com espaço? Não.') == True
assert sem_espacos_inicio_fim(' Começa com espaço? Sim!') == False
assert sem_espacos_inicio_fim('Termina com espaço? Sim! ') == False
assert sem_espacos_inicio_fim(' no início e fim ') == False
