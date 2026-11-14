from dataclasses import dataclass
@dataclass
class Pilha:
    itens: list[int]
    def __len__(self) -> int:
        return len(self.itens)
print(len(Pilha([1, 2])))
