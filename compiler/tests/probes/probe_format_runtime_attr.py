from dataclasses import dataclass
@dataclass
class P:
    x: int
def f(fmt: str) -> str:
    return fmt.format(p=P(3))
print(f("{p.x}"))
