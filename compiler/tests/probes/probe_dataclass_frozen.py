from dataclasses import dataclass
@dataclass(frozen=True)
class C:
    x: int
print(hash(C(1)) == hash(C(1)), {C(1): 1})
