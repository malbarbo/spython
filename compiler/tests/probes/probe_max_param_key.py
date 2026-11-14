from dataclasses import dataclass
@dataclass
class P:
    n: str
    i: int
def melhor(xs: list[P]) -> P:
    return max(xs, key=lambda p: p.i)
print(melhor([P("a", 2)]))
