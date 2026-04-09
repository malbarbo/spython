from enum import Enum, auto

class Maioria(Enum):
    '''Representa o tipo de valores que é a maioria em uma lista de números'''
    POSITIVOS = auto()
    NEGATIVOS = auto()
    NENHUM = auto()

def maioria(lst: list[int]) -> Maioria:
    '''
    Determinar a classe da maioria dos elementos de *lst*.
    Exemplos
    >>> maioria([]).name
    'NENHUM'
    >>> maioria([0, 4, 0, 2, 0]).name
    'POSITIVOS'
    >>> maioria([0, -4, 0, -2, 0]).name
    'NEGATIVOS'
    >>> maioria([0, -4, 0, 2, 0]).name
    'NENHUM'
    '''
    # Conta os positivos e negativos
    positivos = 0
    negativos = 0
    for n in lst:
        if n > 0:
            positivos = positivos + 1
        elif n < 0:
            negativos = negativos + 1

    # Determinia a maioria
    if positivos > negativos:
        maioria = Maioria.POSITIVOS
    elif negativos > positivos:
        maioria = Maioria.NEGATIVOS
    else:
        maioria = Maioria.NENHUM

    return maioria

# Generated from doctests.
assert maioria([]).name == 'NENHUM'
assert maioria([0, 4, 0, 2, 0]).name == 'POSITIVOS'
assert maioria([0, -4, 0, -2, 0]).name == 'NEGATIVOS'
assert maioria([0, -4, 0, 2, 0]).name == 'NENHUM'
