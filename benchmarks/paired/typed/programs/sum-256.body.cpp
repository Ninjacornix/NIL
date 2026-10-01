std::int64_t program(std::array<std::int64_t,256> a) {
    std::int64_t total=0; for(std::int64_t i=0;i<256;i++) total+=get(a,i); return total;
}
