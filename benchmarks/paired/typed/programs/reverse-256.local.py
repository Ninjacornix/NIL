def program(a):
    result = [0]*len(a)
    for i in range(len(a)):
        result[i] = a[len(a)-i-1]
    return result
