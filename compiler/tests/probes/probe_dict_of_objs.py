from dataclasses import dataclass
@dataclass
class P:
    x: int
d: dict[str, P] = {"a": P(1)}
print(d)
