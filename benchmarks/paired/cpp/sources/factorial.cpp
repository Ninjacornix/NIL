#include <cstdint>
using I=std::int64_t;
I program(I n){I r=1;while(n>1){r*=n;--n;}return r;}
