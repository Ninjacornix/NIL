std::array<std::int64_t,256> program(std::array<std::int64_t,256> a) {
    std::array<std::int64_t,256> result{}; for(std::int64_t i=0;i<256;i++) result=put(result,i,get(a,256-i-1)); return result;
}
