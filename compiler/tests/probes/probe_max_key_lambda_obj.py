class P:
    i: int
    def __init__(self, i: int) -> None:
        self.i = i
xs = [P(1), P(3)]
print(max(xs, key=lambda p: p.i).i, min(xs, key=lambda p: p.i).i)
