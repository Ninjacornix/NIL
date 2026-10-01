#include <array>
#include <cstdint>
#include <cstdlib>
template<std::size_t N>
std::int64_t get(const std::array<std::int64_t,N>& a, std::int64_t i) {
    if (i < 0 || static_cast<std::uint64_t>(i) >= N) std::abort();
    return a[static_cast<std::size_t>(i)];
}
std::array<std::int64_t,64> program(std::array<std::int64_t,64> a) {
    for(std::int64_t i=0;i<64;i+=2) { auto old=get(a,i); a[i]=get(a,i+1); a[i+1]=old; } return a;
}
