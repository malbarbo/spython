class Pilha:
    def __init__(self) -> None:
        self.itens: list[int] = []
    def push(self, x: int) -> None:
        self.itens.append(x)
p = Pilha()
p.push(1)
print(p.itens)
