def put(a, i, value):
    result = a.copy()
    result[i] = value
    return result

def program(a):
    result = [0]*len(a)
    for i in range(len(a)):
        result = put(result, i, a[len(a)-i-1])
    return result
