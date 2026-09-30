#include <cstdint>
using I=std::int64_t;
I program(I a,I b){while(b!=0){I c=a-a/b*b;a=b;b=c;}return a;}
