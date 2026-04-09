from dataclasses import dataclass

from enum import Enum, auto

@dataclass
class Posicao:
    '''
    Representa a posição do personagem no jogo
    '''
    x: int
    y: int
    z: int

class Deslocamento(Enum):
    '''
    Representa um deslocamento do personagem no jogo.
    NORTE(+) e SUL(-) correspondem ao eixo x.
    LESTE(+) e OESTE(-) correspondem ao eixo y.
    CIMA(+) e BAIXO(-) correspondem ao eixo z.
    '''
    NORTE = auto()
    SUL = auto()
    LESTE = auto()
    OESTE = auto()
    CIMA = auto()
    BAIXO = auto()

def nova_posicao(p: Posicao, deslocamentos: list[Deslocamento]) -> Posicao:
    '''
    Calcula a nova posição do personagem considerando que ele
    partiu de *p* e realizou os *deslocamentos*.

    Exemplos
    >>> D = Deslocamento
    >>> nova_posicao(Posicao(6, 1, 3), [])
    Posicao(x=6, y=1, z=3)
    >>> nova_posicao(Posicao(6, 1, 3), [D.CIMA, D.NORTE, D.LESTE, D.CIMA, D.CIMA, D.NORTE])
    Posicao(x=8, y=2, z=6)
    >>> nova_posicao(Posicao(6, 1, 3), [D.BAIXO, D.SUL, D.OESTE, D.BAIXO, D.BAIXO, D.SUL])
    Posicao(x=4, y=0, z=0)
    '''
    x = p.x
    y = p.y
    z = p.z
    for d in deslocamentos:
        if d == Deslocamento.NORTE:
            x = x + 1
        elif d == Deslocamento.SUL:
            x = x - 1
        elif d == Deslocamento.LESTE:
            y = y + 1
        elif d == Deslocamento.OESTE:
            y = y - 1
        elif d == Deslocamento.CIMA:
            z = z + 1
        elif d == Deslocamento.BAIXO:
            z = z - 1
    return Posicao(x, y, z)

# Generated from doctests.
D = Deslocamento
assert nova_posicao(Posicao(6, 1, 3), []) == Posicao(x=6, y=1, z=3)
assert nova_posicao(Posicao(6, 1, 3), [D.CIMA, D.NORTE, D.LESTE, D.CIMA, D.CIMA, D.NORTE]) == Posicao(x=8, y=2, z=6)
assert nova_posicao(Posicao(6, 1, 3), [D.BAIXO, D.SUL, D.OESTE, D.BAIXO, D.BAIXO, D.SUL]) == Posicao(x=4, y=0, z=0)
