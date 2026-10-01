std::array<std::int64_t,64> program(std::array<std::int64_t,64> a) {
    for(std::int64_t i=0;i<64;i+=2) { auto old=get(a,i); a[i]=get(a,i+1); a[i+1]=old; } return a;
}
