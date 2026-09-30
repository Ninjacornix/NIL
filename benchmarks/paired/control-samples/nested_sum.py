def program(n):
 r=0
 while n>0:
  s,k=0,n
  while k>0:s,k=s+1,k-1
  r,n=r+s,n-1
 return r
