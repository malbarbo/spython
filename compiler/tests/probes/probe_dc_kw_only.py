from dataclasses import dataclass, field, KW_ONLY
@dataclass(kw_only=True)
class P:
    x: int
    y: int = 0
print(P(x=1))
