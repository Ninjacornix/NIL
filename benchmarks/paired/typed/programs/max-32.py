def program(a):
    best = a[0]
    for i in range(1, len(a)):
        best = a[i] if a[i] > best else best
    return best
