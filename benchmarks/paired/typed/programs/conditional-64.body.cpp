std::array<std::int64_t,64> program(std::array<std::int64_t,64> a) {
    for(std::int64_t i=0;i<64;i++) if(get(a,i)>0) a[i]=get(a,i)+1; return a;
}
