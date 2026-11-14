try:
    raise ValueError("x")
except ValueError as e:
    e.add_note("nota")
    print(e.__notes__)
