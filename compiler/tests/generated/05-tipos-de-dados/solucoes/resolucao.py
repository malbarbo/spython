from dataclasses import dataclass

from enum import Enum, auto

@dataclass
class Resolucao:
    '''
    Representa a resolução de uma imagem ou tela.
    '''
    # A quantidade de linhas de pixel (> 0)
    altura: int
    # A quantidade de colunas de pixels (> 0)
    largura: int

class Aspecto(Enum):
    '''
    Representa o aspecto de uma imagem ou tela, isto é, a proporção entre
    largura e altura.
    '''
    A3x4 = auto()
    A16x9 = auto()
    OUTRO = auto()

def megapixels(r: Resolucao) -> float:
    '''
    Calcula a quantidade de pixels (em megapixels) de uma
    imagem com resolucação r.

    Examples
    >>> megapixels(Resolucao(360, 640))
    0.2304
    >>> megapixels(Resolucao(1024, 768))
    0.786432
    '''
    return r.altura * r.largura / 1000000

def imagem_cabe_tela(i: Resolucao, t: Resolucao) -> bool:
    '''
    Produz True se a imagem com resolucao i pode ser exibida na tela com
    resolucao t sem a necessidade de rotação ou redução de tamanho isso é, se a
    altura da imagem é menor ou igual a altura da tela e a largura da imagem é
    menor ou igual a largura da tela. Produz Falso caso contrário.

    Examples
    >>> imagem_cabe_tela(Resolucao(300, 400), Resolucao(330, 450))
    True
    >>> imagem_cabe_tela(Resolucao(330, 450), Resolucao(330, 450))
    True
    >>> # altura não cabe
    >>> imagem_cabe_tela(Resolucao(331, 400), Resolucao(330, 450))
    False
    >>> # largura não cabe
    >>> imagem_cabe_tela(Resolucao(330, 451), Resolucao(330, 450))
    False
    '''
    return i.altura <= t.altura and i.largura <= t.largura

def aspecto(r: Resolucao) -> Aspecto:
    '''
    Determina o aspecto da resolucao *r*.
    Uma resolução altura x largura tem aspecto x:y se altura * x = largura * y.
    Os valores de x e y considerados são aqueles da definição do tipo Aspecto.

    Exemplos
    >>> aspecto(Resolucao(1024, 768)).name
    'A3x4'
    >>> aspecto(Resolucao(1080, 1920)).name
    'A16x9'
    >>> aspecto(Resolucao(600, 600)).name
    'OUTRO'
    '''
    if r.altura * 3 == r.largura * 4:
        aspecto = Aspecto.A3x4
    elif r.altura * 16 == r.largura * 9:
        aspecto = Aspecto.A16x9
    else:
        aspecto = Aspecto.OUTRO
    return aspecto

# Generated from doctests.
assert megapixels(Resolucao(360, 640)) == 0.2304
assert megapixels(Resolucao(1024, 768)) == 0.786432
assert imagem_cabe_tela(Resolucao(300, 400), Resolucao(330, 450)) == True
assert imagem_cabe_tela(Resolucao(330, 450), Resolucao(330, 450)) == True
assert imagem_cabe_tela(Resolucao(331, 400), Resolucao(330, 450)) == False
assert imagem_cabe_tela(Resolucao(330, 451), Resolucao(330, 450)) == False
assert aspecto(Resolucao(1024, 768)).name == 'A3x4'
assert aspecto(Resolucao(1080, 1920)).name == 'A16x9'
assert aspecto(Resolucao(600, 600)).name == 'OUTRO'
