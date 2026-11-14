class Conta:
    def __init__(self, n: int) -> None:
        self.i = 0
        self.n = n
    def __iter__(self) -> "Conta":
        return self
    def __next__(self) -> int:
        if self.i >= self.n:
            raise StopIteration
        self.i += 1
        return self.i
for x in Conta(3):
    print(x)
