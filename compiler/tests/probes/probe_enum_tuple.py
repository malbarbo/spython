from enum import Enum
class Planeta(Enum):
    TERRA = (5.97e24, 6.37e6)
    MARTE = (6.42e23, 3.39e6)
    def __init__(self, massa: float, raio: float) -> None:
        self.massa = massa
        self.raio = raio
print(Planeta.TERRA.massa)
