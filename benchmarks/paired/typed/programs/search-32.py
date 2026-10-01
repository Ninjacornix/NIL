def program(a, target):
    lo, hi = 0, len(a)
    while lo < hi:
        mid = (lo+hi)//2
        if a[mid] < target:
            lo = mid+1
        else:
            hi = mid
    return lo if lo < len(a) and a[lo] == target else -1
