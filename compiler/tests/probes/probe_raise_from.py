try:
    try:
        int("x")
    except ValueError as e:
        raise KeyError("k") from e
except KeyError as e:
    print("k", e)
