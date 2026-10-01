std::array<std::int64_t,128> program(std::array<std::int64_t,128> a) {
    std::array<std::int64_t,128> result{}; for(std::int64_t i=0;i<128;i++) result=put(result,i,get(a,128-i-1)); return result;
}
