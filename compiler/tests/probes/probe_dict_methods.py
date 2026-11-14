d: dict[str, int] = {}
d.update({"a": 1})
print(d.setdefault("b", 2), d.get("c", 0), d, dict(a=1))
