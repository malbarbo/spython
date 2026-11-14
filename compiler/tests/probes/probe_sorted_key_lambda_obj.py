class P:
    i: int
    def __init__(self, i: int) -> None:
        self.i = i
xs = [P(3), P(1)]
print([p.i for p in sorted(xs, key=lambda p: p.i)])
xs.sort(key=lambda p: -p.i)
print([p.i for p in xs])
