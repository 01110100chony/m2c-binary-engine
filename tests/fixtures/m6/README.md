# Corpus M6

mutations.json contains six handcrafted mutation programs (Apache-2.0), replayed
before generation. M4 programs modify the small known completed job; M5 programs
mutate test-only envelopes derived from the independent OpenSSL fixture. No real
keys or user data. Concrete failures are saved as JSON Case files containing the
actual artifact bytes, not just a PRNG seed; M6_TEST_REPLAY runs such a Case directly.
M5 Case secret bytes are exclusively public test material, never production keys.
The runner records corpus hashes with every campaign. Add minimized real failures
here only after identifying their invariant and documenting their origin.
