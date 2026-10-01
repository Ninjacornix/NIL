def put(a, i, value):
    result = a.copy()
    result[i] = value
    return result

def program(a):
    result = [0]*len(a)
    total = 0
    for i in range(len(a)):
        result = put(result, i, total+a[i])
        total += a[i]
    return result
