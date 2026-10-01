def program(a):
    result = a.copy()
    for i in range(len(a)):
        result[i] = result[i]*3+1
    return result
