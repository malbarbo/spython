d: dict[str, int] = {}
try:
    d["k"]
except KeyError as e:
    print(str(e), repr(e))
