#include <cstdint>
using I=std::int64_t;
I program(I n){I a=0,b=1;while(n>1){I c=a+b;a=b;b=c;--n;}return n==0?a:b;}
