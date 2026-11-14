for i in range(3):
    try:
        if i == 1:
            break
    finally:
        print("f", i)
