class P:
    x: int
    def __init__(self, x: int) -> None:
        self.x = x
    def __lt__(self, o: "P") -> bool:
        return self.x < o.x
    def __repr__(self) -> str:
        return f"P({self.x})"
xs = [P(2), P(1)]
xs.sort()
print(xs, sorted(xs, key=lambda p: -p.x))
