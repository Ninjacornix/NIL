def program(a):
    result = [0]*len(a)
    total = 0
    for i in range(len(a)):
        result[i] = total+a[i]
        total += a[i]
    return result
