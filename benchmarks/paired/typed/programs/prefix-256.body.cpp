std::array<std::int64_t,256> program(std::array<std::int64_t,256> a) {
    std::array<std::int64_t,256> result{}; std::int64_t total=0; for(std::int64_t i=0;i<256;i++) { result=put(result,i,total+get(a,i)); total+=get(a,i); } return result;
}
