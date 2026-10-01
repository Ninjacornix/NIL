std::int64_t program(std::array<std::int64_t,256> a, std::int64_t target) {
    std::int64_t lo=0,hi=256; while(lo<hi) { auto mid=(lo+hi)/2; if(get(a,mid)<target) lo=mid+1; else hi=mid; } return lo<256 && get(a,lo)==target?lo:-1;
}
