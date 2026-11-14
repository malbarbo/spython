d = {"a": 3, "b": 5}
print(sorted(d.items(), key=lambda kv: kv[1], reverse=True))
