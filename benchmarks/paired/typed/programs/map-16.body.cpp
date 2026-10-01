std::array<std::int64_t,16> program(std::array<std::int64_t,16> a) {
    for(std::int64_t i=0;i<16;i++) a[i]=get(a,i)*3+1; return a;
}
