std::int64_t program(std::array<std::int64_t,32> a) {
    std::int64_t best=get(a,0); for(std::int64_t i=1;i<32;i++) best=get(a,i)>best?get(a,i):best; return best;
}
