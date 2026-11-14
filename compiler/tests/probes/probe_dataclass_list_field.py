from dataclasses import dataclass
@dataclass
class T:
    nome: str
    notas: list[float]
print(T("a", [1.0]))
