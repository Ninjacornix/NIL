def program(n):
 a,b=0,1
 while n>1:a,b,n=b,a+b,n-1
 return a if n==0 else b
