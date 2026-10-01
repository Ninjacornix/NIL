std::array<std::int64_t,64> program(std::array<std::int64_t,64> a) {
    for(std::int64_t i=0;i<64;i++) a[i]=get(a,i)*3+1; return a;
}
