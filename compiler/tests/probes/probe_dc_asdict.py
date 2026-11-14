from dataclasses import dataclass, asdict, astuple, fields
@dataclass
class P:
    x: int
print(asdict(P(1)), astuple(P(1)))
