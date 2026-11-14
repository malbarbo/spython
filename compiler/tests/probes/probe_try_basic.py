def parse(s: str) -> int:
    try:
        return int(s)
    except ValueError:
        return -1
print(parse("12"), parse("x"))
