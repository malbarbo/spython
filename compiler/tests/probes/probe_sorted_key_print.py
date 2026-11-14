from dataclasses import dataclass
@dataclass
class P:
    n: str
    i: int
xs = [P("a", 2), P("b", 1)]
print(sorted(xs, key=lambda p: p.i))
