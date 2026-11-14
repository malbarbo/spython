from dataclasses import dataclass
@dataclass
class Ponto:
    x: int
    y: int
def f(p: Ponto) -> str:
    match p:
        case Ponto(0, 0):
            return "origem"
        case Ponto(x=0):
            return "eixo y"
        case Ponto(x, y) if x == y:
            return "diagonal"
        case _:
            return "outro"
print(f(Ponto(0, 0)), f(Ponto(0, 3)), f(Ponto(2, 2)), f(Ponto(1, 2)))
