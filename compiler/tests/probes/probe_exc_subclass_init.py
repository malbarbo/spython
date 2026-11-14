class Erro(ValueError):
    def __init__(self, v: int) -> None:
        super().__init__(f"valor {v}")
        self.v = v
raise Erro(3)
