from dataclasses import dataclass
@dataclass
class No:
    valor: int
    prox: "No | None"
def soma(n: No | None) -> int:
    if n is None:
        return 0
    return n.valor + soma(n.prox)
print(soma(No(1, No(2, None))))
