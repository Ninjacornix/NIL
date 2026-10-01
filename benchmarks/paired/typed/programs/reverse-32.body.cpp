std::array<std::int64_t,32> program(std::array<std::int64_t,32> a) {
    std::array<std::int64_t,32> result{}; for(std::int64_t i=0;i<32;i++) result=put(result,i,get(a,32-i-1)); return result;
}
