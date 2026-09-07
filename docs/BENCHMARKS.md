# M2C Benchmark and Performance Evidence

Status: Post-M6 reproducibility and benchmark evidence pass.  
Canonical evidence: [`docs/evidence/benchmark-full.json`](evidence/benchmark-full.json)  
Runner: [`scripts/benchmark.ps1`](../scripts/benchmark.ps1)  
Date: 2026-09-07

---

## 1. Purpose and Scope

This document provides a disciplined, reproducible technical benchmark of the M2C data engine across milestones M3 (single-file Parquet conversion), M4 (recoverable multipart conversion), M5 (quantum-safe envelope protection), and low-level microbenchmarks.

### Explicit Scope Boundaries
- **Synthetic local evidence**: Benchmarks run on a single developer workstation using deterministic synthetic test datasets; they do not represent a production enterprise SLA.
- **No universal memory bound**: Memory metrics reflect *observed peak working set* on Windows/NTFS, not theoretical or universal heap upper bounds.
- **Source byte throughput**: Throughput (records/s, MiB/s) is calculated strictly from source binary input bytes divided by elapsed wall-clock duration. Binary units (MiB = $1024^2$ bytes) and decimal units (MB = $10^6$ bytes) are distinguished explicitly.
- **No distributed engine comparison**: These measurements evaluate a synchronous, single-process, local-disk Rust engine. They are **not** compared against distributed data systems such as Apache Spark or Cobrix, which operate on clusters with different architectural trade-offs.

---

## 2. Test Environment

Environment automatically captured by `scripts/benchmark.ps1`:

### Host Hardware & OS
- **OS**: Microsoft Windows 10 Pro (Build 19045, 64-bit)
- **Filesystem**: NTFS (Fixed)
- **CPU**: AMD Ryzen 5 3400G with Radeon Vega Graphics (4 physical cores, 8 logical processors)
- **Architecture**: AMD64
- **RAM**: 15.92 GiB physical memory (17,098,760,192 bytes)

### Toolchain & Build Configuration
- **Rust Toolchain**: `rustc 1.95.0 (59807616e 2026-04-14)` / `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`
- **Target**: `x86_64-pc-windows-msvc`
- **Profile**: `release` (Standard Cargo release optimization, no LTO configured in Cargo.toml)
- **Features**: `--all-features` (includes `pqc`)
- **Binary Hash (m2c-pipeline)**: `047939068BC7EBDB1342454CF020A9756B93788582456B3C51E0D819C9522430`
- **Verifier Hash (m6_verify)**: `1B381AE7E6E8FDB1F7C1023834BCBFEE4997779B84F6CD9E4FB1311421964ADC`

---

## 3. Benchmark Methodology

To ensure technical defensibility:

1. **Pre-compiled Release Artifacts**: Binaries are compiled in release mode *before* any timer starts. Cargo compilation and dependency resolution times are strictly excluded. Executables are snapshotted to prevent concurrent cargo invalidation.
2. **Timing Boundaries**:
   - Timers measure only process execution.
   - Dataset generation, keypair generation, and directory setup are performed outside the timed region.
   - Output correctness verification runs *after* timing completes.
3. **Execution Separation**:
   - **Warm-up**: Exactly 1 warm-up run precedes measurement for every scenario (discarded from statistics).
   - **Measured Repetitions**: 5 measured runs per scenario (all raw runs preserved in JSON).
   - **Primary Metric**: The **median** wall-clock elapsed time. Minimum, maximum, and mean are preserved from raw runs.
4. **Distinction of Timings**:
   - `wall_clock_elapsed_ms`: End-to-end execution time captured by high-resolution system stopwatch (`Stopwatch::StartNew()`), including process launch, runtime initialization, and teardown.
   - `internal_elapsed_ms`: High-resolution monotonic elapsed duration reported internally by the CLI `--report-json` telemetry.
5. **Memory Measurement**:
   - `observed_peak_working_set_bytes`: Peak physical working set (`PeakWorkingSet64`) sampled at 10 ms intervals while the process lives.
   - Reported as *observed peak working set sampled by the harness*, recognizing it as an empirical observation on Windows, not a theoretical bound.
6. **Correctness Gates Outside Clock**:
   - Every converted M3 Parquet file is checked against an independent oracle (`m6_verify`) inspecting row count, schema, row group sizing, nullability, and record-by-record value verification.
   - Every M4 directory is validated for lock release, manifest integrity, commit receipts, part count, and Parquet contents.
   - Every M5 protected and unprotected ciphertext/plaintext is verified by exact byte-for-byte roundtrip equality outside the timed region.

---

## 4. End-to-End Conversion Results

Source records are based on `tests/fixtures/sample_fixed.cpy` (record length: 35 bytes, CP037 EBCDIC text, DISPLAY integers/decimals, COMP binary integers/decimals, and COMP-3 packed decimals).

### 4.1 M3 Local Single-File Conversion

In M3, records are decoded in batches and written sequentially as row groups into a single Parquet file.

| Dataset Size | Records | Batch Size | Input Size (MiB) | Median Time (ms) | Min (ms) | Max (ms) | Throughput (rec/s) | Throughput (MiB/s) | Observed Peak WS |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1,000,000 | 1.0M | 256 | 33.38 | 1,314.72 | 1,118.45 | 1,403.22 | 760,619 | 25.39 | 47.77 MiB |
| 1,000,000 | 1.0M | 4,096 | 33.38 | 663.13 | 534.02 | 863.34 | 1,508,007 | 50.34 | 8.31 MiB |
| 1,000,000 | 1.0M | 65,536 | 33.38 | 1,000.44 | 479.98 | 1,644.38 | 999,557 | 33.36 | 14.79 MiB |
| 3,000,000 | 3.0M | 256 | 100.14 | 12,471.72 | 2,592.11 | 21,273.05 | 240,544 | 8.03 | 132.18 MiB |
| 3,000,000 | 3.0M | 4,096 | 100.14 | 2,148.79 | 1,640.99 | 6,304.72 | 1,396,136 | 46.60 | 13.93 MiB |
| 3,000,000 | 3.0M | 65,536 | 100.14 | 2,096.45 | 1,291.01 | 4,596.19 | 1,430,990 | 47.76 | 15.26 MiB |

#### Observations
- **Batch size efficiency**: Larger batch sizes (4,096 and 65,536) achieve throughput of ~1.00M–1.51M records/s.
- **Parquet footer scaling**: Very small batches (256 records) create thousands of row groups ($3,000,000 / 256 = 11,719$ row groups). As specified by the Parquet specification, the Parquet file footer stores metadata for every row group in memory before writing at close. The observed peak memory growth at batch 256 is consistent with accumulating row group footer metadata for thousands of row groups.

---

## 4.2 M4 Recoverable Multipart Conversion

In M4, records are partitioned across discrete, deterministic Parquet part files, each committed via an atomic commit protocol with individual cryptographic receipts in `commits/` and a final `complete.json`.

Tested on 3,000,000 records (100.14 MiB input):

| Records | Batch Size | Output Parts | Median Time (ms) | Min (ms) | Max (ms) | Throughput (rec/s) | Throughput (MiB/s) | Observed Peak WS |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 3,000,000 | 4,096 | 733 | 291,058.86 | 71,131.27 | 376,505.55 | 10,307 | 0.34 | 6.88 MiB |
| 3,000,000 | 65,536 | 46 | 5,049.99 | 4,476.27 | 7,268.12 | 594,061 | 19.83 | 15.24 MiB |

#### Observations
- **Filesystem metadata overhead**: At batch 4,096, M4 creates and atomically commits 733 distinct Parquet files plus 733 JSON commit receipts (1,466 total file creations, flushes, and renames). The observed execution time is consistent with increased filesystem and publication overhead from producing, flushing, and committing many discrete parts and receipts.
- **Batch 65,536**: With 46 parts, M4 throughput reaches ~594,061 records/s (19.83 MiB/s) with an observed peak working set of 15.24 MiB.

---

## 4.3 M5 Post-Quantum Protection (ML-KEM-768 + AES-256-GCM)

Measured on a 64 MiB payload ($67,108,864$ bytes). Keypair generation is conducted prior to measurement. Protect and unprotect are timed independently.

| Operation | Payload Size | Median Time (ms) | Min (ms) | Max (ms) | Throughput (MiB/s) | Observed Peak WS | Correctness Verification |
|---|---:|---:|---:|---:|---:|---:|---|
| **Protect** | 64 MiB | 5,466.58 | 1,945.32 | 7,984.18 | 11.71 | 5.27 MiB | Exact byte-for-byte roundtrip |
| **Unprotect** | 64 MiB | 877.25 | 429.02 | 1,506.76 | 72.95 | 5.27 MiB | Exact byte-for-byte roundtrip |

#### Observations
- **Streaming performance**: Protect median was 5,466.58 ms (11.71 MiB/s); unprotect median was 877.25 ms (72.95 MiB/s).
- **Memory observation**: The streaming design processes fixed-size chunks (1 MiB); the 64 MiB benchmark observed approximately 5.27 MiB (protect) and 5.27 MiB (unprotect) peak working set sampled by the harness on this machine. Observed run-to-run variance across runs is preserved in the evidence without outlier exclusion.

---

## 5. Microbenchmarks (Isolated Compile & Decode)

Microbenchmarks isolated from disk I/O and Parquet writing are executed via `cargo bench --bench m6 -- --profile full` (7 samples per workload, $\ge 250$ ms window, using `std::hint::black_box` and independent RecordBatch validation).

| Workload | Operation | Input / Iteration | Median (ns/it) | Min (ns/it) | Max (ns/it) | Effective Throughput (rec/s) | Effective Throughput (MiB/s) | Effective Throughput (MB/s) |
|---|---|---|---:|---:|---:|---:|---:|---:|
| **Mixed** | Compile Copybook | 402 B copybook | 24,834 | 23,652 | 28,553 | ~40,267 compiles/s | - | - |
| **Mixed** | Decode Batch | 768 records (26.25 KB) | 168,367 | 157,453 | 170,592 | **4.56M records/s** | 152.26 MiB/s | 159.65 MB/s |
| **Numeric** | Compile Copybook | 101 B copybook | 11,673 | 10,983 | 18,837 | ~85,668 compiles/s | - | - |
| **Numeric** | Decode Batch | 256 records (1.75 KB) | 19,208 | 17,947 | 22,631 | **13.33M records/s** | 88.97 MiB/s | 93.29 MB/s |
| **Text** | Compile Copybook | 47 B copybook | 6,078 | 5,789 | 6,432 | ~164,528 compiles/s | - | - |
| **Text** | Decode Batch | 256 records (1.00 KB) | 7,098 | 6,596 | 7,306 | **36.07M records/s** | 137.58 MiB/s | 144.27 MB/s |

> [!IMPORTANT]
> **Microbenchmarks vs. End-to-End**: Microbenchmarks measure in-memory decoding throughput (up to ~4.56M rec/s on mixed records and ~36.07M rec/s on pure text). End-to-end benchmarks include file I/O, Arrow array allocation, Parquet encoding, dictionary creation, and disk flushes (yielding ~1.4M–1.5M rec/s for M3). These two classes of measurements address distinct engineering layers and must not be conflated.

---

## 6. Historical vs. Fresh Comparison

Comparison with historical baseline observations on the same machine architecture (Ryzen 5 3400G / 16 GB DDR4). Deltas are calculated automatically from raw median durations:

| Benchmark Scenario | Historical Observation | Fresh Canonical Measurement | Elapsed Time Delta | Throughput Delta / Notes |
|---|---:|---:|---:|---|
| **M3 3M (batch 65536)** | 2,279.18 ms (~1.32M rec/s) | 2,096.45 ms (1.43M rec/s) | -8.0% | +8.7% throughput (WS: 15.26 MiB) |
| **M3 3M (batch 4096)** | 2,524.80 ms | 2,148.79 ms (1.40M rec/s) | -14.9% | +17.5% throughput (WS: 13.93 MiB) |
| **M3 3M (batch 256)** | 3,401.83 ms | 12,471.72 ms (0.24M rec/s) | +266.6% | -72.7% throughput (WS: 132.18 MiB) |
| **M4 3M (batch 65536)** | 4,163.58 ms (~720k rec/s) | 5,049.99 ms (0.59M rec/s) | +21.3% | -17.6% throughput (WS: 15.24 MiB) |
| **M5 Protect (64 MiB)** | 2,939.66 ms | 5,466.58 ms (11.71 MiB/s) | +86.0% | -46.2% throughput (WS: 5.27 MiB) |
| **M5 Unprotect (64 MiB)** | 1,182.56 ms | 877.25 ms (72.95 MiB/s) | -25.8% | +34.8% throughput (WS: 5.27 MiB) |
| **Micro Mixed Decode** | 171,722 ns/it (4.47M rec/s) | 168,367 ns/it (4.56M rec/s) | -2.0% | +2.0% throughput |

---

## 7. How to Reproduce

### Running the Smoke Verification
```powershell
./scripts/benchmark.ps1 -Profile Smoke
```

### Running the Publication-Grade Full Suite
```powershell
./scripts/benchmark.ps1 -Profile Full -OutputJson docs/evidence/benchmark-full.json
```

### Regenerating and Checking Documentation from JSON
```powershell
# Regenerate BENCHMARKS.md deterministically from JSON evidence
python scripts/render_benchmarks.py

# Validate that documentation is strictly up-to-date with raw evidence (CI gate)
python scripts/render_benchmarks.py --check
```

### Inspecting Raw JSON Output
The raw output preserves exact timings, all individual runs, and environment hashes:
```powershell
Get-Content docs/evidence/benchmark-full.json | ConvertFrom-Json
```

