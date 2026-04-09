from dataclasses import dataclass

@dataclass
class Ponto:
    '''Um ponto no plano cartesiano'''
    x: int
    y: int

@dataclass
class Retangulo:
    '''Um retangulo delimitador.'''
    largura: int
    altura: int

def retangulo_delimitador(pontos: list[Ponto]) -> Retangulo:
    '''
    Determina o retangulo delimitador de altura e largura mínimas que cobre os
    pontos da lista *pontos*.
    Se existe 1 ou menos pontos, o rentagulo terá altura e largura 0.

    Exemplos

                  |
                  |        p5
         p1       |
                  |    p4
                  | p3
    --------------+-------------
              p2  |
                  |   p6
                  |
    >>> p1 = Ponto(-10, 5)
    >>> p2 = Ponto(-3, -1)
    >>> p3 = Ponto(1, 1)
    >>> p4 = Ponto(4, 3)
    >>> p5 = Ponto(9, 8)
    >>> p6 = Ponto(2, -3)
    >>> retangulo_delimitador([])
    Retangulo(largura=0, altura=0)
    >>> retangulo_delimitador([p1])
    Retangulo(largura=0, altura=0)
    >>> retangulo_delimitador([p2, p3])
    Retangulo(largura=4, altura=2)
    >>> retangulo_delimitador([p2, p3, p6])
    Retangulo(largura=5, altura=4)
    >>> retangulo_delimitador([p2, p3, p1, p6, p4, p5])
    Retangulo(largura=19, altura=11)
    '''
    if len(pontos) <= 1:
        r = Retangulo(0, 0)
    else:
        menor_x = pontos[0].x
        maior_x = pontos[0].x
        menor_y = pontos[0].y
        maior_y = pontos[0].y
        for p in pontos:
            if p.x < menor_x:
                menor_x = p.x
            elif p.x > maior_x:
                maior_x = p.x
            if p.y < menor_y:
                menor_y = p.y
            elif p.y > maior_y:
                maior_y = p.y
        r = Retangulo(maior_x - menor_x, maior_y - menor_y)
    return r

# Generated from doctests.
p1 = Ponto(-10, 5)
p2 = Ponto(-3, -1)
p3 = Ponto(1, 1)
p4 = Ponto(4, 3)
p5 = Ponto(9, 8)
p6 = Ponto(2, -3)
assert retangulo_delimitador([]) == Retangulo(largura=0, altura=0)
assert retangulo_delimitador([p1]) == Retangulo(largura=0, altura=0)
assert retangulo_delimitador([p2, p3]) == Retangulo(largura=4, altura=2)
assert retangulo_delimitador([p2, p3, p6]) == Retangulo(largura=5, altura=4)
assert retangulo_delimitador([p2, p3, p1, p6, p4, p5]) == Retangulo(largura=19, altura=11)
