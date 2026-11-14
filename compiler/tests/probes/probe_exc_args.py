try:
    raise ValueError("a", 1)
except ValueError as e:
    print(e.args, e)
