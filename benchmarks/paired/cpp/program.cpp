// Compile one equivalent algorithm with -DCASE=1..8, optionally -DNIL_CHECKED.
#include <cstdint>
#include <limits>
struct Context { std::uint64_t fuel, depth, limit; };
extern "C" [[noreturn]] void nil_fail(std::uint32_t, std::uint64_t, std::uint64_t);
static void fail(unsigned reason) { nil_fail(reason, UINT64_MAX, UINT64_MAX); }
static void tick(Context* c) {
#ifdef NIL_CHECKED
    if (!c->fuel) fail(0);
    --c->fuel;
#endif
}
static std::int64_t literal(Context* c, std::int64_t x) { tick(c); return x; }
static bool greater(Context* c, std::int64_t a, std::int64_t b) { tick(c); return a>b; }
static bool less(Context* c, std::int64_t a, std::int64_t b) { tick(c); return a<b; }
static bool equal(Context* c, std::int64_t a, std::int64_t b) { tick(c); return a==b; }
static bool unequal(Context* c, std::int64_t a, std::int64_t b) { tick(c); return a!=b; }
static std::int64_t add(Context* c, std::int64_t a, std::int64_t b) {
    tick(c);
#ifdef NIL_CHECKED
    std::int64_t r; if (__builtin_add_overflow(a,b,&r)) fail(2); return r;
#else
    return a+b;
#endif
}
static std::int64_t sub(Context* c, std::int64_t a, std::int64_t b) {
    tick(c);
#ifdef NIL_CHECKED
    std::int64_t r; if (__builtin_sub_overflow(a,b,&r)) fail(2); return r;
#else
    return a-b;
#endif
}
static std::int64_t mul(Context* c, std::int64_t a, std::int64_t b) {
    tick(c);
#ifdef NIL_CHECKED
    std::int64_t r; if (__builtin_mul_overflow(a,b,&r)) fail(2); return r;
#else
    return a*b;
#endif
}
static std::int64_t divide(Context* c, std::int64_t a, std::int64_t b) {
    tick(c);
#ifdef NIL_CHECKED
    if (!b) fail(3);
    if (a==INT64_MIN && b==-1) fail(2);
#endif
    return a/b;
}
template<class Yes,class No>
static std::int64_t choose(Context* c, bool condition, Yes yes, No no) {
    tick(c); // If instruction, before lazy region execution.
    auto result=condition ? yes() : no(); tick(c); return result; // Region yield.
}
static bool condition(Context* c, bool value) { tick(c); return value; }
static std::int64_t algorithm(Context* c, std::int64_t a, std::int64_t b, std::int64_t d) {
#if CASE == 1 // factorial: state (result,n)
    auto r=literal(c,1); tick(c);
    while (condition(c,greater(c,a,literal(c,1)))) {
        r=mul(c,r,a); a=sub(c,a,literal(c,1)); tick(c);
    }
    tick(c); return r;
#elif CASE == 2 // Fibonacci: state (a,b,n), simultaneous updates
    auto x=literal(c,0); auto y=literal(c,1); tick(c);
    while (condition(c,greater(c,a,literal(c,1)))) {
        auto next=add(c,x,y); a=sub(c,a,literal(c,1)); x=y; y=next; tick(c);
    }
    auto result=choose(c,equal(c,a,literal(c,0)),[&]{return x;},[&]{return y;});
    tick(c); return result;
#elif CASE == 3
    return choose(c,greater(c,a,b),[&]{return a;},[&]{return b;});
#elif CASE == 4 // counted sum
    auto r=literal(c,0); tick(c);
    while (condition(c,greater(c,a,literal(c,0)))) {
        r=add(c,r,a); a=sub(c,a,literal(c,1)); tick(c);
    }
    tick(c); return r;
#elif CASE == 5
    return choose(c,less(c,a,literal(c,0)),[&]{return sub(c,literal(c,0),a);},[&]{return a;});
#elif CASE == 6
    return choose(c,less(c,a,b),[&]{return b;},[&]{
        return choose(c,greater(c,a,d),[&]{return d;},[&]{return a;});
    });
#elif CASE == 7 // explicit divide/multiply/subtract, not a different modulo algorithm
    tick(c);
    while (condition(c,unequal(c,b,literal(c,0)))) {
        auto q=divide(c,a,b); auto product=mul(c,q,b); auto remainder=sub(c,a,product);
        a=b; b=remainder; tick(c);
    }
    tick(c); return a;
#elif CASE == 8 // nested sum: do not replace the algorithm by a formula
    auto r=literal(c,0); tick(c);
    while (condition(c,greater(c,a,literal(c,0)))) {
        auto s=literal(c,0); auto k=a; tick(c);
        while (condition(c,greater(c,k,literal(c,0)))) {
            s=add(c,s,literal(c,1)); k=sub(c,k,literal(c,1)); tick(c);
        }
        tick(c); r=add(c,r,s); a=sub(c,a,literal(c,1)); tick(c);
    }
    tick(c); return r;
#else
#error CASE must be 1..8
#endif
}
// Exactly the NIL LLVM entry ABI. The shared C driver is compiled separately, no LTO.
#if CASE == 3 || CASE == 7
extern "C" std::int64_t nil_fn0(Context* c, std::uint64_t, std::uint64_t, std::int64_t a, std::int64_t b) {
#elif CASE == 6
extern "C" std::int64_t nil_fn0(Context* c, std::uint64_t, std::uint64_t, std::int64_t a, std::int64_t b, std::int64_t d) {
#else
extern "C" std::int64_t nil_fn0(Context* c, std::uint64_t, std::uint64_t, std::int64_t a) {
#endif
#ifdef NIL_CHECKED
    if (c->depth>=c->limit) fail(1);
    ++c->depth;
#endif
#if CASE == 3 || CASE == 7
    auto result=algorithm(c,a,b,0);
#elif CASE == 6
    auto result=algorithm(c,a,b,d);
#else
    auto result=algorithm(c,a,0,0);
#endif
    tick(c); // Function return.
#ifdef NIL_CHECKED
    --c->depth;
#endif
    return result;
}
