def program(a):
    result = a.copy()
    for i in range(0, len(a), 2):
        result[i], result[i+1] = result[i+1], result[i]
    return result
