# AGENTS.md

# M2C Engineering Instructions

M2C is an experimental systems/data-engineering project built primarily as a student engineering portfolio project.

The project converts legacy mainframe-style fixed-record datasets described by COBOL copybooks into modern typed columnar data, with optional quantum-safe artifact protection in later milestones.

This is **not** intended to be production-grade mainframe infrastructure.

---

## 1. Engineering priorities

When trade-offs exist, prefer, in this order:

1. correctness
2. testability
3. simplicity
4. reproducibility
5. maintainability
6. performance
7. feature breadth

Do not sacrifice correctness or clarity for premature optimization.

Avoid overengineering.

---

## 2. Sources of truth

Before changing architecture or semantics, read the relevant repository documentation:

- `docs/ARCHITECTURE.md`
- `docs/COPYBOOK_SUBSET.md`
- relevant ADRs under `docs/adr/`

The documented architecture and supported COBOL subset are considered frozen for the current milestone.

Do not broaden them implicitly.

If implementation and documentation conflict, stop and report the inconsistency instead of silently choosing a new behavior.

---

## 3. Scope discipline

Work only on the requested milestone.

Do **not** automatically begin the next milestone after completing the current one.

Do not introduce the following unless explicitly requested by the user or required by an approved milestone:

- cloud integrations
- Azure-specific core APIs
- Tokio or async pipelines
- background workers or message queues
- PQC before the crypto milestone
- ML-DSA before its approved milestone
- `OCCURS`
- `REDEFINES`
- variable-length records / RDW / BDW
- broad COBOL compatibility
- UI
- Prometheus/Grafana infrastructure
- AIOps or ML features
- Kubernetes
- microservices
- database or SQL-engine functionality
- distributed coordination
- custom cryptographic primitives

Prefer extending the existing design over introducing new frameworks or abstraction layers.

---

## 4. Copybook and schema rules

The copybook must be parsed and compiled **once**, before record decoding begins.

The decoding hot path must operate on a compiled representation and must not reinterpret COBOL syntax.

The compiled layout should resolve all information required for decoding, including:

- byte offsets
- physical byte lengths
- physical encoding
- signedness
- precision
- scale
- logical Arrow type
- total record length

`FILLER`:

- contributes to offsets and record length
- does not appear in the Arrow schema

Unsupported COBOL syntax must fail explicitly with a typed diagnostic.

Never silently ignore, approximate, or reinterpret unsupported COBOL semantics.

---

## 5. Numeric correctness

Never use floating-point values for COBOL decimal semantics.

Expected logical mappings include:

- `PIC X` → Arrow UTF-8
- unscaled DISPLAY numeric → integer
- scaled DISPLAY numeric → `Decimal128`
- unscaled COMP/BINARY → integer
- scaled COMP/BINARY → `Decimal128`
- COMP-3 / PACKED-DECIMAL → `Decimal128`

Physical COMP/BINARY storage size must follow the documented IBM-style semantics supported by the project.

Do not infer physical byte length directly from decimal digit count without applying the documented storage rules.

Preserve precision and scale explicitly.

---

## 6. External-input safety

Copybooks and binary datasets are untrusted external inputs.

Malformed input must return typed errors instead of causing panics.

Avoid `unwrap()` and `expect()` in runtime paths processing external data.

They are acceptable in tests when the invariant being asserted is intentional and obvious.

Validate:

- bounds
- record lengths
- arithmetic overflow
- offsets
- decimal precision/scale
- invalid encodings
- malformed syntax
- unsupported clauses

Never perform unchecked slicing based on unvalidated external values.

---

## 7. Performance policy

Correctness comes before optimization.

Do not introduce concurrency merely because the pipeline may eventually process large datasets.

The initial file-to-file pipeline is synchronous unless benchmarks justify otherwise.

When optimizing:

1. benchmark
2. profile
3. identify the actual bottleneck
4. optimize that bottleneck
5. measure again

The pipeline must eventually support bounded-memory processing and must never require loading the entire dataset into memory.

Do not pursue zero-copy where it harms simplicity or correctness.

---

## 8. Testing policy

Implementation changes should include or update tests whenever practical.

Prefer independent and deterministic test oracles.

Use:

- unit tests for codecs and parsing rules
- golden tests for copybook AST and compiled schemas
- rejection tests for unsupported syntax
- adversarial tests for malformed input
- property tests where useful
- public independent fixtures for end-to-end correctness
- synthetic datasets primarily for scale and performance testing

A synthetic encoder must not become the only oracle for its matching decoder.

No malformed external input should cause a panic.

---

## 9. Architecture changes

Do not redesign frozen architecture as part of an implementation task.

An architectural change is justified only when there is a concrete blocker involving, for example:

- correctness
- impossible ownership/lifetime requirements
- invalid data semantics
- unrecoverable API design
- security flaw
- inability to satisfy an acceptance criterion

If such a blocker is found:

1. stop implementation of the affected portion
2. describe the concrete problem
3. identify the smallest necessary change
4. explain its impact
5. wait for approval when the change materially alters project contracts

Do not expand scope while fixing architectural problems.

---

## 10. Cryptography policy

Cryptography is an experimental project feature, not a production-security claim.

Do not implement cryptographic primitives manually.

When the crypto milestone is active:

- use established libraries
- use closed, versioned cipher suites
- use ML-KEM only for its intended KEM/key-establishment role
- use an AEAD for bulk payload protection
- preserve nonce uniqueness
- authenticate envelope metadata where required
- validate all lengths and fields before allocation or parsing
- include tamper and wrong-key tests

Do not describe the system as:

- bank-grade
- quantum-proof
- production-ready

without evidence and an explicit approved change in project goals.

---

## 11. Dependency policy

Keep dependencies minimal.

Before adding a dependency, confirm that:

- the standard library or current dependency set does not reasonably solve the problem
- the dependency provides clear value
- its maintenance and portability costs are acceptable

Avoid introducing infrastructure libraries for hypothetical future requirements.

---

## 12. Repository hygiene

Prefer small, coherent changes.

Do not mix unrelated refactors with feature work.

Do not rewrite working code merely for stylistic preference.

Keep public APIs narrow.

Keep milestone-specific implementation details out of generic abstractions until there is a real second use case.

Documentation must describe implemented behavior honestly.

Do not claim planned features as implemented.

---

## 13. Required validation

Before considering an implementation task complete, run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

If one of these commands cannot be executed, report exactly which command failed and why.

Do not hide failing tests or warnings.

Additional milestone-specific validation may be required by the relevant documentation.

---

## 14. Completion report

At the end of a task, provide a concise summary containing:

- what was implemented
- files materially changed
- tests added or changed
- validation commands executed
- whether all validation passed
- remaining limitations
- any deviation from documented architecture
- any unresolved correctness concern

Do not automatically continue into the next milestone.

---

## 15. General rule

When uncertain between a sophisticated solution and a small solution that satisfies the documented contract, prefer the small solution.

This repository should demonstrate disciplined engineering, not maximum feature count.