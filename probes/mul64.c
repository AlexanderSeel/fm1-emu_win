/* Differential probe for the 64-bit multiply-subtract (`fe e1` / `fe f1`, rP -= rA * rB (u|s))
 * and a multiply by an immediate paired with a store (`ea f1 16 40 87 20`, r10 = r4 * 0x16 #
 * [sp] = r7), the forms ADD-1 met (docs/isa-notes.md). Run-time results from `volatile` inputs
 * against the same expressions folded by LLVM, as probes/arith.c:
 *   tools/probe_build.sh mul64
 *   target/release/fm1-emu --bin work/probe_mul64_xip.bin --entry 0x02000120 --dump 0x01C10000 52
 * work/probe_mul64.objdump.txt should show `-= ... (u)` and `(s)` for each msub below. */
#include "harness/probe.h"

#define T(expr_rt, expr_ct) p_check((uint32_t)(expr_rt), (uint32_t)(expr_ct))
#define T64(expr_rt, expr_ct) do { uint64_t g_ = (expr_rt), w_ = (expr_ct); \
    p_check((uint32_t)g_, (uint32_t)w_); p_check((uint32_t)(g_ >> 32), (uint32_t)(w_ >> 32)); } while (0)

__attribute__((noinline)) static uint64_t msub_u(uint64_t acc, uint32_t a, uint32_t b)
{
    return acc - (uint64_t)a * b;
}

__attribute__((noinline)) static int64_t msub_s(int64_t acc, int32_t a, int32_t b)
{
    return acc - (int64_t)a * b;
}

/* two in flight, so the compiler picks other register pairs */
__attribute__((noinline)) static uint64_t msub2(uint64_t x, uint64_t y, int32_t a, int32_t b, uint32_t c, uint32_t d)
{
    x -= (int64_t)a * b;
    y -= (uint64_t)c * d;
    return x ^ (y << 1);
}

/* the shape of ADD-1's store.c: an index times a constant, with stack stores alongside */
__attribute__((noinline)) static uint32_t scaled(uint32_t o, uint32_t *out)
{
    uint32_t first = o * 22u;
    out[0] = first;
    out[1] = first + 22u;
    return first * 3u + 0x1fe00000u * o;
}

static uint32_t sink[2];

int main(void)
{
    volatile uint64_t acc0 = 0, acc1 = 0x0000000100000005ull, accm = 0xFFFFFFFFFFFFFFFFull;
    volatile int64_t sacc = -1234567890123LL;
    volatile uint32_t ua = 0xFFFFFFFFu, ub = 3u, uc = 0x80000000u, ud = 0x12345678u;
    volatile int32_t sa = -2, sb = 0x7FFFFFFF, sc = (int32_t)0x80000000, sd = -7, se = 9;
    volatile uint32_t o1 = 1, o2 = 2, o7 = 7;
    uint32_t buf[2];

    /* unsigned: borrow from the high word, zero, wrap past zero, the largest product */
    T64(msub_u(acc1, ua, ub), 0x0000000100000005ull - 0xFFFFFFFFull * 3u);
    T64(msub_u(acc0, ub, ub), 0ull - 9u);
    T64(msub_u(acc0, ua, ua), 0ull - 0xFFFFFFFFull * 0xFFFFFFFFull);
    T64(msub_u(accm, ua, ua), 0xFFFFFFFFFFFFFFFFull - 0xFFFFFFFFull * 0xFFFFFFFFull);
    T64(msub_u(acc1, uc, ud), 0x0000000100000005ull - 0x80000000ull * 0x12345678ull);
    /* the same bit patterns signed: sign-extended factors */
    T64(msub_s((int64_t)acc1, (int32_t)ua, (int32_t)ub), (int64_t)0x0000000100000005ll - (-1LL * 3));
    T64(msub_s(sacc, sa, sb), -1234567890123LL - (-2LL * 0x7FFFFFFFLL));
    T64(msub_s(sacc, sc, sc), -1234567890123LL - (-2147483648LL * -2147483648LL));
    T64(msub_s(sacc, sc, sb), -1234567890123LL - (-2147483648LL * 0x7FFFFFFFLL));
    T64(msub_s(0, sd, se), 63);
    T64(msub2(acc1, sacc, sd, se, uc, ud),
        (0x0000000100000005ull + 63u) ^ (((uint64_t)-1234567890123LL - 0x80000000ull * 0x12345678ull) << 1));

    /* multiply by an immediate next to stores */
    T(scaled(o1, buf), 66u + 0x1fe00000u);
    T(buf[0], 22u);
    T(buf[1], 44u);
    T(scaled(o2, sink), 132u + 0x3fc00000u);
    T(sink[1], 66u);
    T(scaled(o7, buf), 7u * 66u + 7u * 0x1fe00000u);
    T(o7 * 22u + o2 * 0x100u, 154u + 512u);
    p_done();
    return 0;
}
