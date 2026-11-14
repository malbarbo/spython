try:
    raise ExceptionGroup("g", [ValueError(1)])
except* ValueError:
    print("v")
