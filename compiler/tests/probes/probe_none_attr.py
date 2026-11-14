from dataclasses import dataclass
@dataclass
class N:
    p: "N | None"
n = N(None)
print(n.p.p)
