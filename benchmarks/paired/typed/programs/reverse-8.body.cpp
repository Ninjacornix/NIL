std::array<std::int64_t,8> program(std::array<std::int64_t,8> a) {
    std::array<std::int64_t,8> result{}; for(std::int64_t i=0;i<8;i++) result=put(result,i,get(a,8-i-1)); return result;
}
