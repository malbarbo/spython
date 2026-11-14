class P:
    x: int
    def __init__(self, x: int) -> None:
        self.x = x
    def __eq__(self, o: object) -> bool:
        return isinstance(o, P) and o.x == self.x
print(P(1) in [P(2), P(1)], [P(1)].index(P(1)), [P(1)].count(P(1)))
