from dataclasses import dataclass
@dataclass
class Animal:
    nome: str
class Cao(Animal):
    def fala(self) -> str:
        return self.nome + " au"
print(Cao("rex").fala())
