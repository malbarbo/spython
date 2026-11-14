class Conta:
    saldo: float = 0.0
    def deposita(self, v: float) -> None:
        self.saldo += v
c = Conta()
c.deposita(2.5)
print(c.saldo, Conta.saldo)
