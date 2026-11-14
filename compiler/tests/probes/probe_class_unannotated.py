class Conta:
    def __init__(self, saldo: float) -> None:
        self.saldo = saldo
    def deposita(self, v: float) -> None:
        self.saldo += v
c = Conta(1.0)
c.deposita(2.0)
print(c.saldo)
