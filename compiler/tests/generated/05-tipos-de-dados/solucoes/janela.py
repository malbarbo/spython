from dataclasses import dataclass

@dataclass
class Janela:
    '''
    Representa o espaço que uma janela ocupa em um ambiente gráfico.

    A coordenada (x, y) descreve a posição do canto superior esquerdo. A
    largura representa a quantidade de pixels à direita de (x, y) e a altura
    representa a quantidade de pixels abaixo de (x, y).

    Os valores da largura e altura devem ser maiores que zero.
    '''
    x: int
    y: int
    largura: int
    altura: int

@dataclass
class Clique:
    '''
    Representa a posição de um clique no ambiente gráfico.  Os valores de x e y
    devem ser maiores que 0 e menores do que as dimensões do ambiente.
    '''
    x: int
    y: int

def dentro_janela(j: Janela, c: Clique) -> bool:
    '''
    Devolve True se o clique *c* está dentro do espaço da janela *j*, False contrário.

    Exemplos

    Considere a seguinte janela e os pontos

    x = 100, y = 100, largura = 300, altura = 200

          p5
        +-----------+
    p4  | p1        | p2
        |           |
        +-----------+
          p3
    >>> janela = Janela(100, 100, 300, 200)
    >>> # p1 - dentro da janela
    >>> dentro_janela(janela, Clique(150, 150))
    True
    >>> # p2 - dentro do espaço da altura e depois do espaço da largura
    >>> dentro_janela(janela, Clique(600, 150))
    False
    >>> # p3 - depois do espaço da altura e dentro do espaço da largura
    >>> dentro_janela(janela, Clique(150, 300))
    False
    >>> # p4 - dentro do espaço da altura e antes do espaço da largura
    >>> dentro_janela(janela, Clique(150, 50))
    False
    >>> # p5 - antes do espaço da altura e dentro do espaço da largura
    >>> dentro_janela(janela, Clique(150, 50))
    False
    >>> # canto superior esquerdo
    >>> dentro_janela(janela, Clique(100, 100))
    True
    >>> # canto superior direito
    >>> dentro_janela(janela, Clique(399, 100))
    True
    >>> dentro_janela(janela, Clique(400, 100))
    False
    >>> # canto inferior direito
    >>> dentro_janela(janela, Clique(399, 299))
    True
    >>> dentro_janela(janela, Clique(400, 299))
    False
    >>> dentro_janela(janela, Clique(399, 300))
    False
    >>> dentro_janela(janela, Clique(400, 300))
    False
    >>> # canto inferior esquerdo
    >>> dentro_janela(janela, Clique(100, 299))
    True
    >>> dentro_janela(janela, Clique(100, 300))
    False
    '''
    # c.x está dentro do espaço da largura e c.y dentro do espaço da altura
    return j.x <= c.x < (j.x + j.largura) and j.y <= c.y < (j.y + j.altura)

def janelas_soprepoem(a: Janela, b: Janela) -> bool:
    '''

    Produz True se o espaço das janelas *a* e *b* se soprepõem, False caso contrário.

    Exemplos

    # fixa (eixo y): a janela a vem antes da janela b
    # variável: posição da borda direita de a
    >>> janelas_soprepoem(Janela( 10, 20, 100, 200), Janela(300, 400, 50, 100))
    False
    >>> janelas_soprepoem(Janela(210, 20, 100, 200), Janela(300, 400, 50, 100))
    False
    >>> janelas_soprepoem(Janela(310, 20, 100, 200), Janela(300, 400, 50, 100))
    False
    >>> janelas_soprepoem(Janela(410, 20, 100, 200), Janela(300, 400, 50, 100))
    False

    # fixa: (eixo y) interseção da parte de baixo de a com a parte de cima de b
    # variável: posição da borda direita de a
    >>> janelas_soprepoem(Janela( 10, 250, 100, 200), Janela(300, 400, 50, 100))
    False
    >>> janelas_soprepoem(Janela(210, 250, 100, 200), Janela(300, 400, 50, 100))
    True
    >>> janelas_soprepoem(Janela(310, 250, 100, 200), Janela(300, 400, 50, 100))
    True
    >>> janelas_soprepoem(Janela(410, 250, 100, 200), Janela(300, 400, 50, 100))
    False

    # fixa: (eixo y) interseção da parte de cima de a com a parte de baixo de b
    # variável: posição da borda direita de a
    >>> janelas_soprepoem(Janela( 10, 450, 100, 200), Janela(300, 400, 50, 100))
    False
    >>> janelas_soprepoem(Janela(210, 450, 100, 200), Janela(300, 400, 50, 100))
    True
    >>> janelas_soprepoem(Janela(310, 450, 100, 200), Janela(300, 400, 50, 100))
    True
    >>> janelas_soprepoem(Janela(410, 450, 100, 200), Janela(300, 400, 50, 100))
    False

    # fixa: (eixo y) a janela a vem depois da janela b
    # variável: posição da borda direita de a
    >>> janelas_soprepoem(Janela( 10, 550, 100, 200), Janela(300, 400, 50, 100))
    False
    >>> janelas_soprepoem(Janela(210, 550, 100, 200), Janela(300, 400, 50, 100))
    False
    >>> janelas_soprepoem(Janela(310, 550, 100, 200), Janela(300, 400, 50, 100))
    False
    >>> janelas_soprepoem(Janela(410, 550, 100, 200), Janela(300, 400, 50, 100))
    False
    '''
    # borda direta de a vem antes da borda esquerda de b
    # borda direta de b vem antes da borda esquerda de a
    # borda superior de a vem antes da borda inferior de b
    # borda superior de b vem antes da borda inferior de a
    return a.x < (b.x + b.largura) and \
           b.x < (a.x + a.largura) and \
           a.y < (b.y + b.altura) and \
           b.y < (a.y + a.altura)

# Generated from doctests.
janela = Janela(100, 100, 300, 200)
assert dentro_janela(janela, Clique(150, 150)) == True
assert dentro_janela(janela, Clique(600, 150)) == False
assert dentro_janela(janela, Clique(150, 300)) == False
assert dentro_janela(janela, Clique(150, 50)) == False
assert dentro_janela(janela, Clique(150, 50)) == False
assert dentro_janela(janela, Clique(100, 100)) == True
assert dentro_janela(janela, Clique(399, 100)) == True
assert dentro_janela(janela, Clique(400, 100)) == False
assert dentro_janela(janela, Clique(399, 299)) == True
assert dentro_janela(janela, Clique(400, 299)) == False
assert dentro_janela(janela, Clique(399, 300)) == False
assert dentro_janela(janela, Clique(400, 300)) == False
assert dentro_janela(janela, Clique(100, 299)) == True
assert dentro_janela(janela, Clique(100, 300)) == False
assert janelas_soprepoem(Janela( 10, 20, 100, 200), Janela(300, 400, 50, 100)) == False
assert janelas_soprepoem(Janela(210, 20, 100, 200), Janela(300, 400, 50, 100)) == False
assert janelas_soprepoem(Janela(310, 20, 100, 200), Janela(300, 400, 50, 100)) == False
assert janelas_soprepoem(Janela(410, 20, 100, 200), Janela(300, 400, 50, 100)) == False
assert janelas_soprepoem(Janela( 10, 250, 100, 200), Janela(300, 400, 50, 100)) == False
assert janelas_soprepoem(Janela(210, 250, 100, 200), Janela(300, 400, 50, 100)) == True
assert janelas_soprepoem(Janela(310, 250, 100, 200), Janela(300, 400, 50, 100)) == True
assert janelas_soprepoem(Janela(410, 250, 100, 200), Janela(300, 400, 50, 100)) == False
assert janelas_soprepoem(Janela( 10, 450, 100, 200), Janela(300, 400, 50, 100)) == False
assert janelas_soprepoem(Janela(210, 450, 100, 200), Janela(300, 400, 50, 100)) == True
assert janelas_soprepoem(Janela(310, 450, 100, 200), Janela(300, 400, 50, 100)) == True
assert janelas_soprepoem(Janela(410, 450, 100, 200), Janela(300, 400, 50, 100)) == False
assert janelas_soprepoem(Janela( 10, 550, 100, 200), Janela(300, 400, 50, 100)) == False
assert janelas_soprepoem(Janela(210, 550, 100, 200), Janela(300, 400, 50, 100)) == False
assert janelas_soprepoem(Janela(310, 550, 100, 200), Janela(300, 400, 50, 100)) == False
assert janelas_soprepoem(Janela(410, 550, 100, 200), Janela(300, 400, 50, 100)) == False
