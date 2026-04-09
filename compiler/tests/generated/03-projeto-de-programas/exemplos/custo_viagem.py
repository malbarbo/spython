def custo_viagem(distancia: float, rendimento: float, preco: float) -> float:
    '''
    Calcula o custo em reais para percorrer a *distancia* especificada
    considerando o *rendimento* do carro e o *preco* do litro do combustível.

    Exemplos
    >>> # (120.0 / 10.0) * 5.0
    >>> custo_viagem(120.0, 10.0, 5.0)
    60.0
    >>> # (300.0 / 15.0) * 6.0
    >>> custo_viagem(300.0, 15.0, 6.0)
    120.0
    '''
    return (distancia / rendimento) * preco

# Generated from doctests.
assert custo_viagem(120.0, 10.0, 5.0) == 60.0
assert custo_viagem(300.0, 15.0, 6.0) == 120.0
