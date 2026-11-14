from dataclasses import dataclass
@dataclass
class No:
    v: int
    prox: "No | None" = None
def soma(n: No | None) -> int:
    if n is None:
        return 0
    return n.v + soma(n.prox)
print(soma(No(1, No(2))))
