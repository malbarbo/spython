c: dict[str, int] = {}
for ch in "banana":
    c[ch] = c.get(ch, 0) + 1
print(c)
