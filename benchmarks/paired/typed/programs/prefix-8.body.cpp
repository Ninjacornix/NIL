std::array<std::int64_t,8> program(std::array<std::int64_t,8> a) {
    std::array<std::int64_t,8> result{}; std::int64_t total=0; for(std::int64_t i=0;i<8;i++) { result=put(result,i,total+get(a,i)); total+=get(a,i); } return result;
}
