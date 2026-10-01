std::array<std::int64_t,16> program(std::array<std::int64_t,16> a) {
    for(std::int64_t i=0;i<16;i+=2) { auto old=get(a,i); a[i]=get(a,i+1); a[i+1]=old; } return a;
}
