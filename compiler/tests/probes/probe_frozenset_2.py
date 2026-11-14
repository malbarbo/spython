a = frozenset([1, 2])
s: set[frozenset[int]] = {a}
print(a in s, len(a | frozenset([3])))
