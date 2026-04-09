def quadrado(a: float) -> float:
    '''
    Calcula o quadrado de *a*.

    Exemplo
    >>> quadrado(4.0)
    16.0
    '''
    return a * a

def raiz(a: float) -> float:
    '''
    Calcula a raiz de *a*.
    Requer que *a* seja positivo.

    Exemplo
    >>> raiz(4.0)
    2.0
    '''
    return a ** 0.5

def hipotenusa(a: float, b: float) -> float:
    '''
    Calcula a hipotenusa de um triângulo retângulo com catetos *a* e *b*.
    Requer que *a* e *b* sejam positivos.

    Exemplo
    >>> hipotenusa(3.0, 4.0)
    5.0
    '''
    a2 = quadrado(a)
    b2 = quadrado(b)
    return raiz(a2 + b2)

# Generated from doctests.
assert quadrado(4.0) == 16.0
assert raiz(4.0) == 2.0
assert hipotenusa(3.0, 4.0) == 5.0
