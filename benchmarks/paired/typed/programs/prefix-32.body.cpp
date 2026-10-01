std::array<std::int64_t,32> program(std::array<std::int64_t,32> a) {
    std::array<std::int64_t,32> result{}; std::int64_t total=0; for(std::int64_t i=0;i<32;i++) { result=put(result,i,total+get(a,i)); total+=get(a,i); } return result;
}
