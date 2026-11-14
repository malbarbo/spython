from typing import reveal_type
class No:
    def __init__(self, v: int) -> None:
        self.v = v
        self.prox = None
def liga(a: No, b: No) -> None:
    a.prox = b
a = No(1)
liga(a, No(2))
reveal_type(a.prox)
print(a.prox)
