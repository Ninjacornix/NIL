#include <array>
#include <cstdint>
#include <cstdlib>
template<std::size_t N>
std::int64_t get(const std::array<std::int64_t,N>& a, std::int64_t i) {
    if (i < 0 || static_cast<std::uint64_t>(i) >= N) std::abort();
    return a[static_cast<std::size_t>(i)];
}
std::int64_t program(std::array<std::int64_t,256> a, std::int64_t target) {
    std::int64_t lo=0,hi=256; while(lo<hi) { auto mid=(lo+hi)/2; if(get(a,mid)<target) lo=mid+1; else hi=mid; } return lo<256 && get(a,lo)==target?lo:-1;
}
