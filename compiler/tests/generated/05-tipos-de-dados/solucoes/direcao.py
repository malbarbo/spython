from enum import Enum, auto

class Direcao(Enum):
    '''
    Representa um dos pontos cardiais de acordo com o esquema a seguir

          Norte
            |
    Oeste -   - Leste
            |
           Sul
    '''
    NORTE = auto()
    LESTE = auto()
    SUL = auto()
    OESTE = auto()

def direcao_oposta(d: Direcao) -> Direcao:
    '''Produz a direção oposta de *d*.

    Exemplos
    >>> direcao_oposta(Direcao.NORTE).name
    'SUL'
    >>> direcao_oposta(Direcao.SUL).name
    'NORTE'
    >>> direcao_oposta(Direcao.LESTE).name
    'OESTE'
    >>> direcao_oposta(Direcao.OESTE).name
    'LESTE'
    '''
    if d == Direcao.NORTE:
        do = Direcao.SUL
    elif d == Direcao.SUL:
        do = Direcao.NORTE
    elif d == Direcao.LESTE:
        do = Direcao.OESTE
    elif d == Direcao.OESTE:
        do = Direcao.LESTE
    return do

def direcao_90_horario(d: Direcao) -> Direcao:
    '''
    Devolve a direcao que está a 90 graus no sentido horário de *d*.

    Exemplos
    >>> direcao_90_horario(Direcao.NORTE).name
    'LESTE'
    >>> direcao_90_horario(Direcao.LESTE).name
    'SUL'
    >>> direcao_90_horario(Direcao.SUL).name
    'OESTE'
    >>> direcao_90_horario(Direcao.OESTE).name
    'NORTE'
    '''
    if d == Direcao.NORTE:
        dh = Direcao.LESTE
    elif d == Direcao.LESTE:
        dh = Direcao.SUL
    elif d == Direcao.SUL:
        dh = Direcao.OESTE
    elif d == Direcao.OESTE:
        dh = Direcao.NORTE
    return dh

def direcao_90_anti_horario(d: Direcao) -> Direcao:
    '''
    Devolve a direcao que está a 90 graus no sentido anti-horário de *d*.

    Exemplos
    >>> direcao_90_anti_horario(Direcao.NORTE).name
    'OESTE'
    >>> direcao_90_anti_horario(Direcao.LESTE).name
    'NORTE'
    >>> direcao_90_anti_horario(Direcao.SUL).name
    'LESTE'
    >>> direcao_90_anti_horario(Direcao.OESTE).name
    'SUL'
    '''
    return direcao_90_horario(direcao_90_horario(direcao_90_horario(d)))

# Generated from doctests.
assert direcao_oposta(Direcao.NORTE).name == 'SUL'
assert direcao_oposta(Direcao.SUL).name == 'NORTE'
assert direcao_oposta(Direcao.LESTE).name == 'OESTE'
assert direcao_oposta(Direcao.OESTE).name == 'LESTE'
assert direcao_90_horario(Direcao.NORTE).name == 'LESTE'
assert direcao_90_horario(Direcao.LESTE).name == 'SUL'
assert direcao_90_horario(Direcao.SUL).name == 'OESTE'
assert direcao_90_horario(Direcao.OESTE).name == 'NORTE'
assert direcao_90_anti_horario(Direcao.NORTE).name == 'OESTE'
assert direcao_90_anti_horario(Direcao.LESTE).name == 'NORTE'
assert direcao_90_anti_horario(Direcao.SUL).name == 'LESTE'
assert direcao_90_anti_horario(Direcao.OESTE).name == 'SUL'
