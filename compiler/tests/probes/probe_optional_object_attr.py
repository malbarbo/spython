class N:
    def __init__(self, v: int) -> None:
        self.v = v
        self.prox: N | None = None
n = N(1)
n.prox = N(2)
print(n.prox.v if n.prox is not None else 0)
