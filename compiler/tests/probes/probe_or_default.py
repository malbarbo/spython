def f(nome: str) -> str:
    return nome or "anon"
print(f(""), f("a"))
