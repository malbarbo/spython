try:
    {"a": 1}["b"]
except Exception as e:
    print("pegou", repr(e))
