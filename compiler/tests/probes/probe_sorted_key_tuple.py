xs = ["bb", "a", "ccc"]
print(sorted(xs, key=lambda s: (len(s), s)))
