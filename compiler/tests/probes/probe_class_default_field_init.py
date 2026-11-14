class Contador:
    n: int = 0
    def __init__(self, n: int) -> None:
        self.n = n
print(Contador(3).n)
