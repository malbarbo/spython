def f(s: str) -> list[str]:
    return sorted(s.split(), key=len)
print(f("aaa b cc"))
