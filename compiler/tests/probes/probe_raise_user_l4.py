class SaldoInsuficiente(Exception):
    pass
def saca(s: float, v: float) -> float:
    if v > s:
        raise SaldoInsuficiente("sem saldo")
    return s - v
print(saca(1.0, 2.0))
