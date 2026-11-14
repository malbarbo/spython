from enum import Enum, auto
class Cor(Enum):
    VERMELHO = auto()
    VERDE = auto()
def f(c: Cor) -> str:
    match c:
        case Cor.VERMELHO:
            return "pare"
        case Cor.VERDE:
            return "siga"
print(f(Cor.VERDE))
