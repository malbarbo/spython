from dataclasses import dataclass

@dataclass
class Desempenho:
    '''
    Um time e seu desempenho em um campeonato de futebol.
    '''
    time: str
    # quantidade de pontos (3 pontos por vitória e 1 por empate)
    pontos: int
    # quantidade de jogos ganhos
    vitorias: int
    # diferença entre os gols marcados e sofridos
    saldo: int

def melhor_desempenho(a: Desempenho, b: Desempenho) -> str:
    '''
    Devolve o nome do time com melhor desempenho entre *a* e *b*.

    O melhor desempenho é aquele tem a maior quantidade de pontos;
    em caso de empate, aquele com maior número de vitórias, em
    caso de empate o maior saldo de gols e em caso de empate,
    aquele que o time vem antes em ordem alfabética.

    Requer que os times de *a* e *b* sejam diferentes.

    Exemplos
    >>> melhor_desempenho(Desempenho('Maringá', 4, 1, 2), Desempenho('Londrina', 1, 0, -2))
    'Maringá'
    >>> melhor_desempenho(Desempenho('Maringá', 4, 1, 2), Desempenho('Londrina', 4, 2, 2))
    'Londrina'
    >>> melhor_desempenho(Desempenho('Maringá', 5, 2, 2), Desempenho('Londrina', 5, 2, 1))
    'Maringá'
    >>> melhor_desempenho(Desempenho('Maringá', 5, 2, 2), Desempenho('Londrina', 5, 2, 2))
    'Londrina'
    '''
    if a.pontos > b.pontos or \
            a.pontos == b.pontos and a.vitorias > b.vitorias or \
            a.pontos == b.pontos and a.vitorias == b.vitorias and a.saldo > b.saldo or \
            a.pontos == b.pontos and a.vitorias == b.vitorias and a.saldo == b.saldo and a.time < b.time:
        time = a.time
    else:
        time = b.time
    return time

def atualiza_desempenho(d: Desempenho, gols_marcados: int, gols_sofridos: int) -> Desempenho:
    '''
    Devolve um novo desempenho atualizado a partir de *d* considerando que o
    time fez *gol_marcados* gols e o adiversário *gol_sofridos* gols.

    Se o time ganhou (marcou mais gols que sofreu), então o número de pontos
    aumenta em 3 e vitórias em 1.

    Se o time empatou (marcou e sofreu a mesma quantidade de gols), então o
    número de pontos aumenta em 1.

    O saldo de gols é sempre atualizado somando *gols_marcados* e
    substraíndo *gols_sofridos*.

    Requer que gols_marcados e gols_sofridos sejam não negativos.

    Exemplos
    >>> atualiza_desempenho(Desempenho('Maringa', 5, 1, -1), 4, 2)
    Desempenho(time='Maringa', pontos=8, vitorias=2, saldo=1)
    >>> atualiza_desempenho(Desempenho('Maringa', 8, 2, 1), 3, 3)
    Desempenho(time='Maringa', pontos=9, vitorias=2, saldo=1)
    >>> atualiza_desempenho(Desempenho('Maringa', 8, 2, 1), 1, 4)
    Desempenho(time='Maringa', pontos=8, vitorias=2, saldo=-2)
    '''
    pontos = d.pontos
    vitorias = d.vitorias
    saldo = d.saldo + gols_marcados - gols_sofridos
    if gols_marcados > gols_sofridos:
        pontos = pontos + 3
        vitorias = vitorias + 1
    elif gols_marcados == gols_sofridos:
        pontos = pontos + 1
    return Desempenho(d.time, pontos, vitorias, saldo)

# Generated from doctests.
assert melhor_desempenho(Desempenho('Maringá', 4, 1, 2), Desempenho('Londrina', 1, 0, -2)) == 'Maringá'
assert melhor_desempenho(Desempenho('Maringá', 4, 1, 2), Desempenho('Londrina', 4, 2, 2)) == 'Londrina'
assert melhor_desempenho(Desempenho('Maringá', 5, 2, 2), Desempenho('Londrina', 5, 2, 1)) == 'Maringá'
assert melhor_desempenho(Desempenho('Maringá', 5, 2, 2), Desempenho('Londrina', 5, 2, 2)) == 'Londrina'
assert atualiza_desempenho(Desempenho('Maringa', 5, 1, -1), 4, 2) == Desempenho(time='Maringa', pontos=8, vitorias=2, saldo=1)
assert atualiza_desempenho(Desempenho('Maringa', 8, 2, 1), 3, 3) == Desempenho(time='Maringa', pontos=9, vitorias=2, saldo=1)
assert atualiza_desempenho(Desempenho('Maringa', 8, 2, 1), 1, 4) == Desempenho(time='Maringa', pontos=8, vitorias=2, saldo=-2)
