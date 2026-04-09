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

@dataclass
class Resultado:
    '''
    O resultado de um jogo.
    '''
    gols_marcados: int
    gols_sofridos: int

def calcula_desempenho(time: str, resultados: list[Resultado]) -> Desempenho:
    '''
    Calcula o desempenho do *time* considerando os *resultados* dos seu jogos.

    Veja uma descrição de como o desempenho é calculado na função
    atualiza_desempenho.

    Exemplos
    >>> calcula_desempenho('Atlético MG', [])
    Desempenho(time='Atlético MG', pontos=0, vitorias=0, saldo=0)
    >>> calcula_desempenho('Flamengo', [Resultado(4, 1), Resultado(3, 3), Resultado(0, 1)])
    Desempenho(time='Flamengo', pontos=4, vitorias=1, saldo=2)
    '''
    d = Desempenho(time, 0, 0, 0)
    for resultado in resultados:
        d = atualiza_desempenho(d, resultado.gols_marcados, resultado.gols_sofridos)
    return d

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
assert calcula_desempenho('Atlético MG', []) == Desempenho(time='Atlético MG', pontos=0, vitorias=0, saldo=0)
assert calcula_desempenho('Flamengo', [Resultado(4, 1), Resultado(3, 3), Resultado(0, 1)]) == Desempenho(time='Flamengo', pontos=4, vitorias=1, saldo=2)
assert atualiza_desempenho(Desempenho('Maringa', 5, 1, -1), 4, 2) == Desempenho(time='Maringa', pontos=8, vitorias=2, saldo=1)
assert atualiza_desempenho(Desempenho('Maringa', 8, 2, 1), 3, 3) == Desempenho(time='Maringa', pontos=9, vitorias=2, saldo=1)
assert atualiza_desempenho(Desempenho('Maringa', 8, 2, 1), 1, 4) == Desempenho(time='Maringa', pontos=8, vitorias=2, saldo=-2)
