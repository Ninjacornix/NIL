#include <cstdint>
using I=std::int64_t;
I program(I n){I r=0;while(n>0){I s=0,k=n;while(k>0){++s;--k;}r+=s;--n;}return r;}
