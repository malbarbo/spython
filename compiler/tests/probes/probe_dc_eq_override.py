from dataclasses import dataclass
@dataclass
class A:
    n: int
    def __eq__(self, o: object) -> bool:
        return isinstance(o, A) and o.n % 10 == self.n % 10
print(A(1) == A(11))
