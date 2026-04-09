from dataclasses import dataclass

@dataclass
class Aluno:
    '''
    Representa um aluno e o resultado que ele obteve em uma disciplina.

    Requer que media esteja entre 0 e 10.
    Requer que frequencia esteja entre 0 e 100.
    '''
    nome: str
    media: float
    frequencia: float

def aprovados(alunos: list[Aluno]) -> list[str]:
    '''
    Determina o nome dos *alunos* que foram aprovados, isto é, obteveram média
    >= 6 e frequência >= 75

    Exemplos

    >>> aprovados([])
    []
    >>> aprovados([
    ...     Aluno('Alfredo', 6.0, 74.0),
    ...     Aluno('Bianca', 5.9, 75.0),
    ...     Aluno('Jorge', 6.0, 75.0),
    ...     Aluno('Leonidas', 5.9, 74.0),
    ...     Aluno('Maria', 8.0, 90.0)])
    ['Jorge', 'Maria']
    '''
    aprovados = []
    for aluno in alunos:
        if aluno.media >= 6 and aluno.frequencia >= 75:
            aprovados.append(aluno.nome)
    return aprovados

# Generated from doctests.
assert aprovados([]) == []
assert aprovados([
    Aluno('Alfredo', 6.0, 74.0),
    Aluno('Bianca', 5.9, 75.0),
    Aluno('Jorge', 6.0, 75.0),
    Aluno('Leonidas', 5.9, 74.0),
    Aluno('Maria', 8.0, 90.0)]) == ['Jorge', 'Maria']
