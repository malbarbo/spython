class No:
    def __init__(self, v: int) -> None:
        self.v = v
        self.prox = None
n = No(1)
n.prox = No(2)
print(n.prox is not None)
