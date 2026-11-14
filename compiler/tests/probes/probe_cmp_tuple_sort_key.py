xs = ["bb", "a", "cc"]
print(sorted(xs, key=lambda s: (len(s), s)))
