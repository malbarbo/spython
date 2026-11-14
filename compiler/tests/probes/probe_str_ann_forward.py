from dataclasses import dataclass
@dataclass
class No:
    v: int
    prox: "No | None"
print(No(1, None))
