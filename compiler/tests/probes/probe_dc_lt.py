from dataclasses import dataclass
@dataclass
class A:
    nota: float
    def __lt__(self, o: "A") -> bool:
        return self.nota < o.nota
print(sorted([A(2.0), A(1.0)]))
