class SemSaldo(Exception):
    pass
def saca(s: float, v: float) -> float:
    if v > s:
        raise SemSaldo()
    return s - v
print(saca(1.0, 0.5))
