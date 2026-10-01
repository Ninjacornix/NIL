#include <array>
#include <cstdint>
#include <cstdlib>
template<std::size_t N>
std::int64_t get(const std::array<std::int64_t,N>& a, std::int64_t i) {
    if (i < 0 || static_cast<std::uint64_t>(i) >= N) std::abort();
    return a[static_cast<std::size_t>(i)];
}
std::int64_t program(std::array<std::int64_t,256> a) {
    std::int64_t best=get(a,0); for(std::int64_t i=1;i<256;i++) best=get(a,i)>best?get(a,i):best; return best;
}
