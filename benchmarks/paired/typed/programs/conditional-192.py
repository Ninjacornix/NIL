def program(a):
    result = a.copy()
    for i in range(len(a)):
        if result[i] > 0:
            result[i] += 1
    return result
