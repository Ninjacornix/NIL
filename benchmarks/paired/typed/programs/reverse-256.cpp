#include <array>
#include <cstdint>
#include <cstdlib>
template<std::size_t N>
std::int64_t get(const std::array<std::int64_t,N>& a, std::int64_t i) {
    if (i < 0 || static_cast<std::uint64_t>(i) >= N) std::abort();
    return a[static_cast<std::size_t>(i)];
}
template<std::size_t N>
std::array<std::int64_t,N> put(std::array<std::int64_t,N> a, std::int64_t i, std::int64_t v) {
    if (i < 0 || static_cast<std::uint64_t>(i) >= N) std::abort();
    a[static_cast<std::size_t>(i)] = v;
    return a;
}
std::array<std::int64_t,256> program(std::array<std::int64_t,256> a) {
    std::array<std::int64_t,256> result{}; for(std::int64_t i=0;i<256;i++) result=put(result,i,get(a,256-i-1)); return result;
}
