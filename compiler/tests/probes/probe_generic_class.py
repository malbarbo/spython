class Pilha[T]:
    def __init__(self) -> None:
        self.itens: list[T] = []
    def empilha(self, x: T) -> None:
        self.itens.append(x)
    def desempilha(self) -> T:
        return self.itens.pop()
p = Pilha[int]()
p.empilha(1)
print(p.desempilha())
