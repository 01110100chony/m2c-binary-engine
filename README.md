# M2C Quantum-Safe Data Pipeline

An experimental pipeline, written primarily in Rust, for studying the conversion of legacy mainframe binary data into typed columnar data:

```text
fixed-record binary file + COBOL copybook
    -> compiled layout
    -> typed decoding
    -> Arrow / Parquet
    -> optional quantum-safe protection
    -> local or cloud sink
```

## Status

This is an educational and portfolio project maintained by a Computer Engineering student. The v0.1 architecture is frozen. **M0 through M6 are implemented:** foundation, copybook compiler, codecs and Arrow, local Parquet conversion, multipart recovery, experimental artifact protection, and local technical evidence.

The project provides synchronous local conversion from a fixed-record file to Parquet, in bounded batches, through both the library and the CLI. M4 adds deterministic multipart output with a manifest and restart after process interruption, while preserving the M3 single-output conversion path. M5 adds standalone file protection under the optional `pqc` feature using ML-KEM-768, HKDF-SHA-256, and AES-256-GCM/STREAM-BE32. M6 adds per-command JSON reporting, complementary campaigns, external verification, and reproducible benchmarks. Cloud and observability infrastructure remain future work. The software should not be used for sensitive data or production workloads.

## M6 Local Evidence

Add `--report-json` to existing commands to obtain result, duration, and observable
volume information on stdout; diagnostics and exit codes are preserved.
Unknown fields are `null`; the report contains no paths or keys.
The PowerShell 7 runner provides `Verify`, `Demo`, `Fuzz`, and `Bench`:

```powershell
./scripts/m6.ps1 -Mode Verify
./scripts/m6.ps1 -Mode Demo
./scripts/m6.ps1 -Mode Fuzz -Profile Full
./scripts/m6.ps1 -Mode Bench -Profile Full
```

See the [evidence contract and reproduction instructions](docs/M6_EVIDENCE.md) and the
[local results](docs/M6_RESULTS.md). Local gates passed; the reparse/symlink case
was skipped because of Windows environment error 1314. The Windows workflow passed remotely on commit
`8d44218605a59a190590772fa52232c5859c9bc8` with Verify/Fuzz Smoke/Demo/Bench Smoke;
that run predates the final remediation, which was validated locally. Full remains local.
The measurements do not establish an SLA or a globally constant memory bound.

## Performance — Local Benchmarks

Empirical measurements on a local machine (AMD Ryzen 5 3400G, 16 GB DDR4, Windows 10/NTFS, Rust 1.95 MSVC release, 1 warmup + 5 measured runs, no LTO):

- **M3 (direct Parquet conversion)**: 3,000,000 records (100.14 MiB) at batch size 65,536 with a median of **2,096.45 ms** (~1.43M rec/s, 47.76 MiB/s input throughput, 15.26 MiB observed peak working set).
- **M4 (recoverable multipart conversion)**: 3,000,000 records at batch size 65,536 (46 parts) with a median of **5,049.99 ms** (~594.1k rec/s, 19.83 MiB/s, 15.24 MiB observed peak working set).
- **M5 (quantum-safe protection)**: 64 MiB payload with protect at **5,466.58 ms** (11.71 MiB/s, 5.27 MiB WS) and unprotect at **877.25 ms** (72.95 MiB/s, 5.27 MiB WS), verified by strict byte-for-byte roundtrip equality.
- **Microbenchmarks (in-memory, isolated from disk/Parquet)**: mixed-batch decoding at **4.56M rec/s** (168,367 ns/it for 768 records, 152.26 MiB/s) and text-only decoding at **36.07M rec/s** (7,098 ns/it for 256 records, 137.58 MiB/s).

See [BENCHMARKS.md](docs/BENCHMARKS.md) for the complete methodology, limits, and reproducibility details. To run the reproducible harness:

```powershell
./scripts/benchmark.ps1 -Profile Full
```

## External Compatibility — Spark / Cobrix Validation

Independent differential validation of semantic correctness (not a performance benchmark):

- Processing of a realistic externally generated GnuCOBOL fixture (`input.ebcdic`, 100 records of 24 bytes, 2,400 bytes total, pinned SHA-256, with CP037 text and COMP-3 packed decimals; not extracted from production).
- Independent decoding with the official **AbsaOSS Cobrix 2.9.4** connector on **Apache Spark 4.0.1** (Java 17, Ubuntu 24.04 LTS via WSL2).
- Field-by-field semantic comparison through [`scripts/compare_cobrix.py`](scripts/compare_cobrix.py): **100/100 identical records** after explicit schema and decimal-scale validation (`int32` vs `decimal128(9,0)` for ID, `decimal128(9,2)` for value).
- This is not a performance test, does not use proprietary production data, and does not imply universal COBOL compatibility. See [EXTERNAL_COMPATIBILITY.md](docs/EXTERNAL_COMPATIBILITY.md) for the full report and formal instructions.

## Conversion Foundation

M1 transforms a copybook from the documented subset into a compiled representation. M2 uses that layout to decode bytes without reinterpreting COBOL in the hot path:

```text
sample.cpy
    -> fixed-format normalization
    -> parser
    -> minimal AST
    -> CompiledCopybook
         - record length
         - field offsets and byte lengths
         - physical encodings and signedness
         - precision and scale
         - logical Arrow types
         - Arrow Schema
```

The accepted subset is intentionally small. Syntax outside that subset must produce an explicit diagnostic with source location and must never be silently ignored. See [COPYBOOK_SUBSET.md](docs/COPYBOOK_SUBSET.md) for the complete contract.

## Architecture v0.1

The repository uses a single Rust package with both a library and CLI. The flow is:

1. parse and compile the copybook once;
2. in M3, read a fixed-record binary file in bounded batches;
3. use the M2 codecs to decode each batch into Arrow;
4. in M3, incrementally write row groups into a single local Parquet file;
5. in M4, provide local parts, immutable commit receipts, and explicit resume;
6. in M5, optionally protect an already produced file using AEAD + ML-KEM;
7. only after the local demonstration, consider an object-storage sink.

The limits and invariants are described in [ARCHITECTURE.md](docs/ARCHITECTURE.md). The analysis that motivated the rebuild remains in [ANALISE_DO_PROJETO.md](docs/ANALISE_DO_PROJETO.md).

## Frozen Logical Mapping

| COBOL field | Arrow logical type |
|---|---|
| `PIC X...` | `Utf8` |
| integer DISPLAY | `Int64` |
| DISPLAY with implicit `V` scale | `Decimal128` |
| COMP/BINARY without scale | `Int64` |
| COMP/BINARY with implicit `V` scale | `Decimal128` |
| COMP-3/PACKED-DECIMAL | `Decimal128` |

`FILLER` consumes bytes and participates in offsets and record length, but is not exposed in the Arrow Schema.

## Development Verification

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo test --all-targets --all-features
cargo test --doc
cargo test --doc --all-features
```

M1 tests validate the AST, layout, and rejection of unsupported syntax. M2 adds the complete public CP037 table, an annotated binary fixture compared against an expected `RecordBatch`, adversarial tests, and fixed-seed properties. See the [fixture provenance](tests/fixtures/README.md).

The milestone entry-point API is `parse_and_compile_copybook(&str)`. To inspect the two stages separately, use `parse_copybook(&str)` followed by `compile_copybook(&CopybookAst)`.

## M2 Decoding

```rust
use m2c_pipeline::{parse_and_compile_copybook, RecordDecoder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let layout = parse_and_compile_copybook(
        "       01 ROOT.\n       05 COUNT-FIELD PIC 9(2).\n"
    )?;
    let decoder = RecordDecoder::try_new(&layout)?;
    let batch = decoder.decode_batch(&[0xF1, 0xF2, 0xF0, 0xF3])?;
    assert_eq!(batch.num_rows(), 2); // Int64: 12 and 3
    Ok(())
}
```

The decoder validates the layout once and can be reused. The caller supplies bounded
batches containing complete records. Text preserves spaces and CP037 control characters;
numeric errors return typed diagnostics with no partial batch. Sign, precision, capacity,
and position policies are defined in the [decoding contract](docs/DECODING.md).

## M3 Local Conversion

```bash
cargo run -- convert --copybook tests/fixtures/sample_fixed.cpy --input tests/fixtures/sample_fixed.bin --output sample.parquet --batch-records 2
```

All four arguments are required. `--batch-records` must be a positive integer
and limits the number of records read and decoded at a time. The example
produces three rows in two row groups (2 + 1), with no additional compression.
The output must be a new path with an existing parent directory; existing files
are never overwritten. Errors are written to stderr and return a non-zero status.

The library exposes
`convert_file(&CompiledCopybook, &Path, &Path, usize) -> Result<(), ConversionError>`.
The paths are input and output, in that order; the final argument limits the
records per batch. The copybook is compiled once and a single `RecordDecoder`
is reused. M2 schema, names, order, types, precision/scale, and values are preserved.
Tests reopen the Parquet file for validation; the CLI does not perform a mandatory
second read of the result.

Empty input produces an empty Parquet file with schema. Incomplete records at EOF,
zero batch size, overflow, and invalid numeric data return typed errors. Decoding
errors report the absolute file position and global record index; the byte offset
inside the M2 context remains batch-relative. FILLER-only layouts are rejected by M3
conversion without changing their support in the compiler or decoder.

Data memory is bounded by batch size; the Parquet footer accumulates metadata
proportional to the number of row groups. A failure can leave a partial output;
there is no atomic commit, manifest, retry, or resume. The `local_conversion`
test runs the CLI against the known fixture with a two-record batch, reopens the
output, and compares schema and values against constants independent of the decoder.

## M4 Recoverable Conversion

```bash
cargo run -- convert-parts --copybook tests/fixtures/sample_fixed.cpy --input tests/fixtures/sample_fixed.bin --output-dir sample-parts --batch-records 2
cargo run -- convert-parts --copybook tests/fixtures/sample_fixed.cpy --input tests/fixtures/sample_fixed.bin --output-dir sample-parts --batch-records 2 --resume
```

All four arguments are required in both modes. Without `--resume`, the
output directory must be new and its parent must already exist; with the flag,
the directory must exist. One batch corresponds to one Parquet part, with
deterministic names and record ranges. The example produces two parts (2 + 1 records).
Empty input produces one part with schema and zero rows.

The library exposes `convert_parts(&CompiledCopybook, &Path, &Path, usize,
RecoveryMode) -> Result<(), RecoveryError>`, with `Create` and `Resume` modes.
`manifest.json` identifies the conversion; each published part receives an immutable
receipt under `commits/`; `complete.json` marks completion. A Parquet file without
a receipt is an orphan, not a commit. Resume validates the input, layout, configuration,
and every committed part before cleaning staging or regenerating the next orphan.

SHA-256 identity uses the full input content and the canonical layout/schema.
Identical input at a different path can resume; changed input, a different layout,
or a different batch size requires a new destination. Missing or corrupted committed
parts cause an error and are never regenerated automatically. Resuming an already
completed conversion revalidates the result without rewriting committed parts.

The initial target is Windows/MSVC with local NTFS and Rust 1.89 or newer. A file
lock prevents concurrent M4 invocations against the same destination. Staging and
publication remain on the same filesystem; the guarantee covers process failure,
without promising durability after a power loss or operating-system failure. The
input and managed directory must remain immutable to other programs during each
invocation. Hashes provide identity/integrity and do not protect the payload.

The [M4 contract](docs/M4_RECOVERY.md) defines the format, bootstrap, recovery,
invariants, fault injection, acceptance criteria, and limitations. Data and hashing
use bounded memory; artifacts and metadata on disk grow with the number of parts.
Resume validation rereads the input and committed parts.

## M5 Experimental Protection

M5 is compiled only with the `pqc` feature and operates separately from the M4 pipeline:

```bash
cargo run --features pqc -- keygen --output-dir sample-keys
cargo run --features pqc -- protect --input sample.parquet --public-key sample-keys/public.key --output sample.parquet.m5
cargo run --features pqc -- unprotect --input sample.parquet.m5 --secret-key sample-keys/secret.key --output recovered.parquet
```

`keygen` requires a non-existent destination directory. `protect` and `unprotect`
require an existing parent directory and never overwrite the final name. The M5 v1
publication guarantee covers only Windows/MSVC on a local NTFS volume: staging and
destination remain in the same directory and commit uses atomic hard-link creation,
failing if the final name already exists. Other filesystems, shares, and platforms
fail closed. No M5 operation writes into an M4-managed namespace; an M4 artifact
may only be used as a read-only input.

The frozen v1 suite uses ML-KEM-768 for key establishment, HKDF-SHA-256, and
AES-256-GCM in STREAM-BE32, with 1 MiB chunks and the complete header as AAD for
each frame. The formal limit is `2^32` frames and `2^52` plaintext bytes. Production
obtains all entropy from the operating system. `recipient_public_key_sha256` is only
a fingerprint/identifier for the public-key representation; its integrity comes from
authenticated AAD and it does not authenticate the recipient's identity.

The library exposes `generate_keypair`, `protect_file`, and `unprotect_file` under
`m2c_pipeline::protection`. The operations process the payload with bounded memory,
publish only after complete validation, and return typed errors, permission warnings,
and staging-residue status. Restrictive permissions and zeroization of secret buffers
owned by M2C are best-effort mitigations. Secret-key protection at rest, signatures,
multiple recipients, KMS/HSM integration, M4 integration, and cloud support remain
out of scope.

The binary format, failure model, limits, and normative limitations are defined in
the [frozen M5 contract](docs/M5_PROTECTION.md).

`keygen` publishes each file atomically, but does not provide a transaction for the
keypair as a whole: `public.key` is published before `secret.key`. If the second
publication fails, the operation returns an error and preserves the already published
public key; the partial directory is neither adopted nor overwritten on a later run
and requires manual handling. Cleanup of M2C-owned staging files is best-effort.
A public residue after commit may also remain. In this error case there is no
`KeyGenerationOutcome`, so warnings and the status of the first commit are not
returned separately. The no-partial-publication guarantee applies to each individual
file, not to the pair as a transaction.

During `unprotect`, authenticated plaintext from earlier frames may exist in staging
before the complete file has been authenticated. On normal error returns, `Drop`
attempts to remove that staging file on a best-effort basis. Abrupt process termination
or power loss before commit can leave `.m2c-m5-staging-*` containing partial plaintext,
without publishing the final destination. The destination is published only after
complete authentication and size validation. Post-crash staging cleanup/recovery and
resume are outside M5; there is no additional guarantee against local access to staging
during the operation or after a crash.

## Roadmap

- **M0 — foundation:** honest status and documentation, modules and contracts compatible with architecture v0.1, clean local CI.
- **M1 — copybook compiler:** fixed-format normalization, subset parser, minimal AST, compiled layout, Arrow Schema, and diagnostics.
- **M2 — codecs and Arrow:** CP037, DISPLAY, COMP/BINARY, COMP-3, and typed `RecordBatch` production.
- **M3 — local Core MVP:** fixed-record source, bounded-memory batches, conversion CLI, and local Parquet writing/validation.
- **M4 — robustness and recovery:** deterministic parts, manifest, atomic commit, fault injection, and resume.
- **M5 — experimental PQC protection (implemented):** AEAD for the payload and ML-KEM for key establishment/protection, with a frozen versioned suite.
- **M6 — technical evidence and demo:** local observability, expanded fuzzing, reproducible benchmarks, and documented demonstration.
- **M7 — optional extensions:** object-storage/cloud sink, ML-DSA, and new formats only after the local portfolio version.

The project does not aim to implement complete COBOL, replace IBM tooling, create a database engine, or provide enterprise infrastructure.

## Documentation

- [Architecture v0.1](docs/ARCHITECTURE.md)
- [COBOL Copybook Subset v0.1](docs/COPYBOOK_SUBSET.md)
- [M2 Record Decoding](docs/DECODING.md)
- [M4 Local Recovery](docs/M4_RECOVERY.md)
- [M5 Experimental Protection](docs/M5_PROTECTION.md)
- [Benchmarks and Performance](docs/BENCHMARKS.md)
- [External Spark/Cobrix Validation](docs/EXTERNAL_COMPATIBILITY.md)
- [Initial Project Analysis](docs/ANALISE_DO_PROJETO.md)

## References

- [Apache Arrow](https://arrow.apache.org/)
- [Apache Parquet](https://parquet.apache.org/)
- [NIST FIPS 203 — ML-KEM](https://csrc.nist.gov/pubs/fips/203/final)
- [IBM Enterprise COBOL documentation](https://www.ibm.com/docs/en/cobol-zos)
