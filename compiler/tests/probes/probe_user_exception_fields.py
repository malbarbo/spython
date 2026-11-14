class SaldoInsuficiente(Exception):
    falta: float
    def __init__(self, falta: float) -> None:
        super().__init__(f"faltam {falta}")
        self.falta = falta

try:
    raise SaldoInsuficiente(2.5)
except SaldoInsuficiente as e:
    print(e.falta, e)
