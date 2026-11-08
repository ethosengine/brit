/* Probe unguarded C compilation, not the assembly in runtime-dispatched functions.
 * This complements the Rust feature allow-list and release execution tests. */
#if !defined(__x86_64__) && !defined(_M_X64)
#error "Brit CPU baseline probe requires an x86-64 C compiler for this target"
#endif
#if defined(__AVX__) || defined(__AVX2__) || defined(__AVX512F__) || \
    defined(__BMI__) || defined(__BMI2__) || defined(__FMA__) || defined(__F16C__) || \
    defined(__AES__) || defined(__PCLMUL__) || defined(__SHA__) || defined(__LZCNT__) || \
    defined(__RDRND__) || defined(__RDSEED__) || defined(__ADX__) || defined(__GFNI__) || \
    defined(__VAES__) || defined(__VPCLMULQDQ__) || defined(__SSE4A__) || defined(__XOP__) || \
    defined(__FMA4__) || defined(__TBM__) || defined(__MOVBE__)
#error "Brit CPU baseline is x86-64-v2; C compiler enables unsupported global instructions"
#endif
int brit_cpu_baseline_probe;
