std::int64_t program(std::array<std::int64_t,32> a, std::array<std::int64_t,32> b) {
    std::int64_t total=0; for(std::int64_t i=0;i<32;i++) total+=get(a,i)*get(b,i); return total;
}
