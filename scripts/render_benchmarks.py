#!/usr/bin/env python3
"""Deterministic benchmark report generator and validator.

Derives human-facing benchmark tables in docs/BENCHMARKS.md directly from
canonical raw benchmark JSON (e.g. docs/evidence/benchmark-full.json).

Usage:
    python scripts/render_benchmarks.py [--json <path>] [--output <path>]
    python scripts/render_benchmarks.py --check [--json <path>] [--output <path>]
"""

import argparse
import difflib
import json
import math
import os
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

HISTORICAL_BASELINES = [
    {
        "key": "m3_3m_65536",
        "scenario": "**M3 3M (batch 65536)**",
        "hist_label": "2,279.18 ms (~1.32M rec/s)",
        "hist_ms": 2279.18,
        "hist_recs_s": 1316262.87,
        "hist_ws_mib": 15.39,
        "benchmark": "m3-convert",
        "records": 3000000,
        "batch": 65536,
    },
    {
        "key": "m3_3m_4096",
        "scenario": "**M3 3M (batch 4096)**",
        "hist_label": "2,524.80 ms",
        "hist_ms": 2524.80,
        "hist_recs_s": 1188212.93,
        "hist_ws_mib": 13.88,
        "benchmark": "m3-convert",
        "records": 3000000,
        "batch": 4096,
    },
    {
        "key": "m3_3m_256",
        "scenario": "**M3 3M (batch 256)**",
        "hist_label": "3,401.83 ms",
        "hist_ms": 3401.83,
        "hist_recs_s": 881878.28,
        "hist_ws_mib": 131.69,
        "benchmark": "m3-convert",
        "records": 3000000,
        "batch": 256,
    },
    {
        "key": "m4_3m_65536",
        "scenario": "**M4 3M (batch 65536)**",
        "hist_label": "4,163.58 ms (~720k rec/s)",
        "hist_ms": 4163.58,
        "hist_recs_s": 720533.77,
        "hist_ws_mib": 15.29,
        "benchmark": "m4-convert-parts",
        "records": 3000000,
        "batch": 65536,
    },
    {
        "key": "m5_protect_64mb",
        "scenario": "**M5 Protect (64 MiB)**",
        "hist_label": "2,939.66 ms",
        "hist_ms": 2939.66,
        "hist_mib_s": 21.77,
        "hist_ws_mib": 5.28,
        "benchmark": "m5-protect",
        "payload_bytes": 67108864,
    },
    {
        "key": "m5_unprotect_64mb",
        "scenario": "**M5 Unprotect (64 MiB)**",
        "hist_label": "1,182.56 ms",
        "hist_ms": 1182.56,
        "hist_mib_s": 54.12,
        "hist_ws_mib": 5.29,
        "benchmark": "m5-unprotect",
        "payload_bytes": 67108864,
    },
    {
        "key": "micro_mixed_decode",
        "scenario": "**Micro Mixed Decode**",
        "hist_label": "171,722 ns/it (4.47M rec/s)",
        "hist_ns": 171722,
        "hist_recs_s": 4472344.84,
        "workload": "mixed",
        "operation": "decode",
    },
]


def validate_json_schema(data: Dict[str, Any]) -> None:
    """Validate required schema fields in benchmark JSON."""
    required_root = [
        "schema_version",
        "generated_at",
        "profile",
        "git",
        "host",
        "toolchain",
        "benchmarks",
        "microbenchmarks",
    ]
    for k in required_root:
        if k not in data:
            raise ValueError(f"Missing required root field in benchmark JSON: '{k}'")

    git = data["git"]
    for k in ["commit", "branch", "dirty"]:
        if k not in git:
            raise ValueError(f"Missing required field in git metadata: '{k}'")

    toolchain = data["toolchain"]
    for k in ["target", "build_profile", "binary_sha256", "verifier_sha256"]:
        if k not in toolchain:
            raise ValueError(f"Missing required field in toolchain metadata: '{k}'")

    if not isinstance(data["benchmarks"], list) or len(data["benchmarks"]) == 0:
        raise ValueError("Field 'benchmarks' must be a non-empty list.")

    for i, b in enumerate(data["benchmarks"]):
        for k in ["benchmark", "command", "runs", "median_wall_clock_elapsed_ms"]:
            if k not in b:
                raise ValueError(f"Benchmark entry {i} missing required field: '{k}'")
        if not isinstance(b["runs"], list) or len(b["runs"]) == 0:
            raise ValueError(f"Benchmark entry {i} ({b['benchmark']}) has no runs.")


def compute_median(values: List[float]) -> float:
    sorted_v = sorted(values)
    n = len(sorted_v)
    if n % 2 == 1:
        return sorted_v[n // 2]
    return (sorted_v[n // 2 - 1] + sorted_v[n // 2]) / 2.0


def format_num(val: Optional[float], decimals: int = 2) -> str:
    if val is None:
        return "N/A"
    return f"{val:,.{decimals}f}"


def format_int(val: Optional[int]) -> str:
    if val is None:
        return "N/A"
    return f"{val:,}"


def render_markdown(
    data: Dict[str, Any],
    json_rel_path: str = "docs/evidence/benchmark-full.json",
    out_dir: str = "docs",
) -> str:
    validate_json_schema(data)

    git = data["git"]
    host = data["host"]
    toolchain = data["toolchain"]
    benchmarks = data["benchmarks"]
    microbenchmarks = data["microbenchmarks"]

    # Extract Host details safely
    cpu_name = host.get("cpu_name") or host.get("cpu_model", "N/A")
    cores = host.get("number_of_cores", "N/A")
    log_proc = host.get("number_of_logical_processors", host.get("logical_processors", "N/A"))
    total_ram_bytes = host.get("total_physical_memory_bytes") or host.get("total_memory_bytes", 0)
    total_ram_gib = host.get("total_physical_memory_gib") or host.get("total_memory_gib", round(total_ram_bytes / (1024**3), 2))
    os_desc = host.get("os_caption") or host.get("os", "Windows")
    os_build = host.get("os_build_number", "")
    os_version = host.get("os_version", "")
    os_str = f"{os_desc} (Build {os_build}, 64-bit)" if os_build else f"{os_desc} ({os_version})"
    filesystem = host.get("filesystem", "NTFS")
    volume_type = host.get("volume_type", "Fixed volume")
    storage_str = f"{filesystem} ({volume_type})"

    # Rustc & Cargo
    rustc_ver = toolchain.get("rustc_verbose", "").splitlines()[0] if toolchain.get("rustc_verbose") else "rustc"
    cargo_ver = toolchain.get("cargo", "cargo")
    target = toolchain.get("target", "x86_64-pc-windows-msvc")
    binary_sha = toolchain.get("binary_sha256", "")
    verifier_sha = toolchain.get("verifier_sha256", "")

    # Date
    gen_at = data.get("generated_at", "")
    date_str = gen_at[:10] if gen_at else "2026-09-07"

    # Profile notes
    toolchain_note = toolchain.get("note", "Standard Cargo release optimization (opt-level 3, no LTO configured in Cargo.toml)")

    try:
        json_href = os.path.relpath(json_rel_path, out_dir).replace("\\", "/")
    except Exception:
        json_href = "evidence/benchmark-full.json"

    lines: List[str] = [
        "# M2C Benchmark and Performance Evidence",
        "",
        "Status: Post-M6 reproducibility and benchmark evidence pass.  ",
        f"Canonical evidence: [`{json_rel_path}`]({json_href})  ",
        "Runner: [`scripts/benchmark.ps1`](../scripts/benchmark.ps1)  ",
        f"Date: {date_str}",
        "",
        "---",
        "",
        "## 1. Purpose and Scope",
        "",
        "This document provides a disciplined, reproducible technical benchmark of the M2C data engine across milestones M3 (single-file Parquet conversion), M4 (recoverable multipart conversion), M5 (quantum-safe envelope protection), and low-level microbenchmarks.",
        "",
        "### Explicit Scope Boundaries",
        "- **Synthetic local evidence**: Benchmarks run on a single developer workstation using deterministic synthetic test datasets; they do not represent a production enterprise SLA.",
        "- **No universal memory bound**: Memory metrics reflect *observed peak working set* on Windows/NTFS, not theoretical or universal heap upper bounds.",
        "- **Source byte throughput**: Throughput (records/s, MiB/s) is calculated strictly from source binary input bytes divided by elapsed wall-clock duration. Binary units (MiB = $1024^2$ bytes) and decimal units (MB = $10^6$ bytes) are distinguished explicitly.",
        "- **No distributed engine comparison**: These measurements evaluate a synchronous, single-process, local-disk Rust engine. They are **not** compared against distributed data systems such as Apache Spark or Cobrix, which operate on clusters with different architectural trade-offs.",
        "",
        "---",
        "",
        "## 2. Test Environment",
        "",
        "Environment automatically captured by `scripts/benchmark.ps1`:",
        "",
        "### Host Hardware & OS",
        f"- **OS**: {os_str}",
        f"- **Filesystem**: {storage_str}",
        f"- **CPU**: {cpu_name} ({cores} physical cores, {log_proc} logical processors)",
        f"- **Architecture**: {host.get('architecture', 'AMD64')}",
        f"- **RAM**: {total_ram_gib:.2f} GiB physical memory ({total_ram_bytes:,} bytes)",
        "",
        "### Toolchain & Build Configuration",
        f"- **Rust Toolchain**: `{rustc_ver}` / `{cargo_ver}`",
        f"- **Target**: `{target}`",
        f"- **Profile**: `release` ({toolchain_note})",
        "- **Features**: `--all-features` (includes `pqc`)",
        f"- **Binary Hash (m2c-pipeline)**: `{binary_sha}`",
        f"- **Verifier Hash (m6_verify)**: `{verifier_sha}`",
        "",
        "---",
        "",
        "## 3. Benchmark Methodology",
        "",
        "To ensure technical defensibility:",
        "",
        "1. **Pre-compiled Release Artifacts**: Binaries are compiled in release mode *before* any timer starts. Cargo compilation and dependency resolution times are strictly excluded. Executables are snapshotted to prevent concurrent cargo invalidation.",
        "2. **Timing Boundaries**:",
        "   - Timers measure only process execution.",
        "   - Dataset generation, keypair generation, and directory setup are performed outside the timed region.",
        "   - Output correctness verification runs *after* timing completes.",
        "3. **Execution Separation**:",
        "   - **Warm-up**: Exactly 1 warm-up run precedes measurement for every scenario (discarded from statistics).",
        "   - **Measured Repetitions**: 5 measured runs per scenario (all raw runs preserved in JSON).",
        "   - **Primary Metric**: The **median** wall-clock elapsed time. Minimum, maximum, and mean are preserved from raw runs.",
        "4. **Distinction of Timings**:",
        "   - `wall_clock_elapsed_ms`: End-to-end execution time captured by high-resolution system stopwatch (`Stopwatch::StartNew()`), including process launch, runtime initialization, and teardown.",
        "   - `internal_elapsed_ms`: High-resolution monotonic elapsed duration reported internally by the CLI `--report-json` telemetry.",
        "5. **Memory Measurement**:",
        "   - `observed_peak_working_set_bytes`: Peak physical working set (`PeakWorkingSet64`) sampled at 10 ms intervals while the process lives.",
        "   - Reported as *observed peak working set sampled by the harness*, recognizing it as an empirical observation on Windows, not a theoretical bound.",
        "6. **Correctness Gates Outside Clock**:",
        "   - Every converted M3 Parquet file is checked against an independent oracle (`m6_verify`) inspecting row count, schema, row group sizing, nullability, and record-by-record value verification.",
        "   - Every M4 directory is validated for lock release, manifest integrity, commit receipts, part count, and Parquet contents.",
        "   - Every M5 protected and unprotected ciphertext/plaintext is verified by exact byte-for-byte roundtrip equality outside the timed region.",
        "",
        "---",
        "",
        "## 4. End-to-End Conversion Results",
        "",
        "Source records are based on `tests/fixtures/sample_fixed.cpy` (record length: 35 bytes, CP037 EBCDIC text, DISPLAY integers/decimals, COMP binary integers/decimals, and COMP-3 packed decimals).",
        "",
        "### 4.1 M3 Local Single-File Conversion",
        "",
        "In M3, records are decoded in batches and written sequentially as row groups into a single Parquet file.",
        "",
        "| Dataset Size | Records | Batch Size | Input Size (MiB) | Median Time (ms) | Min (ms) | Max (ms) | Throughput (rec/s) | Throughput (MiB/s) | Observed Peak WS |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]

    m3_entries = [b for b in benchmarks if b["benchmark"] == "m3-convert"]
    # Sort by records, then batch
    m3_entries.sort(key=lambda x: (x.get("records", 0), x.get("batch_records", 0)))

    for b in m3_entries:
        recs = b["records"]
        recs_label = f"{recs / 1_000_000:.1f}M"
        batch = b["batch_records"]
        input_bytes = b["input_bytes"]
        input_mib = input_bytes / (1024 * 1024)

        runs = b["runs"]
        wall_times = [r["wall_clock_elapsed_ms"] for r in runs]
        med = compute_median(wall_times)
        min_t = min(wall_times)
        max_t = max(wall_times)

        # Throughput
        rec_s = (recs * 1000.0) / med if med > 0 else 0
        mib_s = (input_mib * 1000.0) / med if med > 0 else 0

        # Peak WS
        peak_ws_bytes = max(r.get("observed_peak_working_set_bytes") or 0 for r in runs)
        peak_ws_mib = peak_ws_bytes / (1024 * 1024)

        lines.append(
            f"| {recs:,} | {recs_label} | {batch:,} | {input_mib:.2f} | {med:,.2f} | {min_t:,.2f} | {max_t:,.2f} | {int(round(rec_s)):,} | {mib_s:.2f} | {peak_ws_mib:.2f} MiB |"
        )

    # Dynamic M3 batch observation
    large_batch_thru = [
        (b["records"] * 1000.0) / compute_median([r["wall_clock_elapsed_ms"] for r in b["runs"]])
        for b in m3_entries
        if b.get("batch_records") in (4096, 65536)
    ]
    if large_batch_thru:
        min_lb = min(large_batch_thru) / 1_000_000
        max_lb = max(large_batch_thru) / 1_000_000
        obs_m3_batch = f"- **Batch size efficiency**: Larger batch sizes (4,096 and 65,536) achieve throughput of ~{min_lb:.2f}M–{max_lb:.2f}M records/s."
    else:
        obs_m3_batch = "- **Batch size efficiency**: Larger batch sizes achieve significantly higher throughput."

    lines.extend([
        "",
        "#### Observations",
        obs_m3_batch,
        "- **Parquet footer scaling**: Very small batches (256 records) create thousands of row groups ($3,000,000 / 256 = 11,719$ row groups). As specified by the Parquet specification, the Parquet file footer stores metadata for every row group in memory before writing at close. The observed peak memory growth at batch 256 is consistent with accumulating row group footer metadata for thousands of row groups.",
        "",
        "---",
        "",
        "## 4.2 M4 Recoverable Multipart Conversion",
        "",
        "In M4, records are partitioned across discrete, deterministic Parquet part files, each committed via an atomic commit protocol with individual cryptographic receipts in `commits/` and a final `complete.json`.",
        "",
        "Tested on 3,000,000 records (100.14 MiB input):",
        "",
        "| Records | Batch Size | Output Parts | Median Time (ms) | Min (ms) | Max (ms) | Throughput (rec/s) | Throughput (MiB/s) | Observed Peak WS |",
        "|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
    ])

    m4_entries = [b for b in benchmarks if b["benchmark"] == "m4-convert-parts"]
    m4_entries.sort(key=lambda x: (x.get("records", 0), x.get("batch_records", 0)))

    for b in m4_entries:
        recs = b["records"]
        batch = b["batch_records"]
        parts = b.get("parts") or math.ceil(recs / batch)
        input_bytes = b["input_bytes"]
        input_mib = input_bytes / (1024 * 1024)

        runs = b["runs"]
        wall_times = [r["wall_clock_elapsed_ms"] for r in runs]
        med = compute_median(wall_times)
        min_t = min(wall_times)
        max_t = max(wall_times)

        rec_s = (recs * 1000.0) / med if med > 0 else 0
        mib_s = (input_mib * 1000.0) / med if med > 0 else 0

        peak_ws_bytes = max(r.get("observed_peak_working_set_bytes") or 0 for r in runs)
        peak_ws_mib = peak_ws_bytes / (1024 * 1024)

        lines.append(
            f"| {recs:,} | {batch:,} | {int(parts)} | {med:,.2f} | {min_t:,.2f} | {max_t:,.2f} | {int(round(rec_s)):,} | {mib_s:.2f} | {peak_ws_mib:.2f} MiB |"
        )

    m4_65k = next((b for b in m4_entries if b.get("batch_records") == 65536), None)
    if m4_65k:
        m4_65k_med = compute_median([r["wall_clock_elapsed_ms"] for r in m4_65k["runs"]])
        m4_65k_recs_s = (m4_65k["records"] * 1000.0) / m4_65k_med
        m4_65k_mib_s = ((m4_65k["input_bytes"] / (1024 * 1024)) * 1000.0) / m4_65k_med
        m4_65k_ws = max(r.get("observed_peak_working_set_bytes") or 0 for r in m4_65k["runs"]) / (1024 * 1024)
        obs_m4_batch = f"- **Batch 65,536**: With 46 parts, M4 throughput reaches ~{int(round(m4_65k_recs_s)):,} records/s ({m4_65k_mib_s:.2f} MiB/s) with an observed peak working set of {m4_65k_ws:.2f} MiB."
    else:
        obs_m4_batch = "- **Batch 65,536**: Larger batch sizes yield significantly higher throughput and fewer parts."

    lines.extend([
        "",
        "#### Observations",
        "- **Filesystem metadata overhead**: At batch 4,096, M4 creates and atomically commits 733 distinct Parquet files plus 733 JSON commit receipts (1,466 total file creations, flushes, and renames). The observed execution time is consistent with increased filesystem and publication overhead from producing, flushing, and committing many discrete parts and receipts.",
        obs_m4_batch,
        "",
        "---",
        "",
        "## 4.3 M5 Post-Quantum Protection (ML-KEM-768 + AES-256-GCM)",
        "",
        "Measured on a 64 MiB payload ($67,108,864$ bytes). Keypair generation is conducted prior to measurement. Protect and unprotect are timed independently.",
        "",
        "| Operation | Payload Size | Median Time (ms) | Min (ms) | Max (ms) | Throughput (MiB/s) | Observed Peak WS | Correctness Verification |",
        "|---|---:|---:|---:|---:|---:|---:|---|",
    ])

    m5_protect = next((b for b in benchmarks if b["benchmark"] == "m5-protect"), None)
    m5_unprotect = next((b for b in benchmarks if b["benchmark"] == "m5-unprotect"), None)

    p_med, p_mib_s, p_ws_mib = 0.0, 0.0, 0.0
    u_med, u_mib_s, u_ws_mib = 0.0, 0.0, 0.0

    if m5_protect:
        runs = m5_protect["runs"]
        wall_times = [r["wall_clock_elapsed_ms"] for r in runs]
        p_med = compute_median(wall_times)
        min_t = min(wall_times)
        max_t = max(wall_times)
        size_bytes = m5_protect["payload_bytes"]
        size_mib = size_bytes / (1024 * 1024)
        p_mib_s = (size_mib * 1000.0) / p_med if p_med > 0 else 0
        peak_ws_bytes = max(r.get("observed_peak_working_set_bytes") or 0 for r in runs)
        p_ws_mib = peak_ws_bytes / (1024 * 1024)
        lines.append(
            f"| **Protect** | {int(size_mib)} MiB | {p_med:,.2f} | {min_t:,.2f} | {max_t:,.2f} | {p_mib_s:.2f} | {p_ws_mib:.2f} MiB | Exact byte-for-byte roundtrip |"
        )

    if m5_unprotect:
        runs = m5_unprotect["runs"]
        wall_times = [r["wall_clock_elapsed_ms"] for r in runs]
        u_med = compute_median(wall_times)
        min_t = min(wall_times)
        max_t = max(wall_times)
        size_bytes = m5_unprotect["payload_bytes"]
        size_mib = size_bytes / (1024 * 1024)
        u_mib_s = (size_mib * 1000.0) / u_med if u_med > 0 else 0
        peak_ws_bytes = max(r.get("observed_peak_working_set_bytes") or 0 for r in runs)
        u_ws_mib = peak_ws_bytes / (1024 * 1024)
        lines.append(
            f"| **Unprotect** | {int(size_mib)} MiB | {u_med:,.2f} | {min_t:,.2f} | {max_t:,.2f} | {u_mib_s:.2f} | {u_ws_mib:.2f} MiB | Exact byte-for-byte roundtrip |"
        )

    obs_m5_perf = f"- **Streaming performance**: Protect median was {p_med:,.2f} ms ({p_mib_s:.2f} MiB/s); unprotect median was {u_med:,.2f} ms ({u_mib_s:.2f} MiB/s)."
    obs_m5_mem = f"- **Memory observation**: The streaming design processes fixed-size chunks (1 MiB); the 64 MiB benchmark observed approximately {p_ws_mib:.2f} MiB (protect) and {u_ws_mib:.2f} MiB (unprotect) peak working set sampled by the harness on this machine. Observed run-to-run variance across runs is preserved in the evidence without outlier exclusion."

    lines.extend([
        "",
        "#### Observations",
        obs_m5_perf,
        obs_m5_mem,
        "",
        "---",
        "",
        "## 5. Microbenchmarks (Isolated Compile & Decode)",
        "",
        r"Microbenchmarks isolated from disk I/O and Parquet writing are executed via `cargo bench --bench m6 -- --profile full` (7 samples per workload, $\ge 250$ ms window, using `std::hint::black_box` and independent RecordBatch validation).",
        "",
        "| Workload | Operation | Input / Iteration | Median (ns/it) | Min (ns/it) | Max (ns/it) | Effective Throughput (rec/s) | Effective Throughput (MiB/s) | Effective Throughput (MB/s) |",
        "|---|---|---|---:|---:|---:|---:|---:|---:|",
    ])

    for m in microbenchmarks:
        wl = m["workload"].capitalize()
        op = m["operation"].capitalize()
        op_label = f"{op} Copybook" if op == "Compile" else f"{op} Batch"
        med_ns = m["median_ns_per_iteration"]
        min_ns = m["min_ns_per_iteration"]
        max_ns = m["max_ns_per_iteration"]
        recs_per_it = m.get("records_per_iteration")
        bytes_per_it = m.get("input_bytes_per_iteration") or 0

        # Input label
        if op == "Compile":
            input_label = f"{bytes_per_it} B copybook"
            thru_rec_s = f"~{int(round(1e9 / med_ns)):,} compiles/s" if med_ns > 0 else "N/A"
            thru_mib_s = "-"
            thru_mb_s = "-"
        else:
            input_label = f"{recs_per_it} records ({bytes_per_it / 1024:.2f} KB)"
            rec_rate = (recs_per_it * 1e9) / med_ns if med_ns > 0 and recs_per_it else 0
            thru_rec_s = f"**{rec_rate / 1_000_000:.2f}M records/s**"

            # MiB/s (1024^2) and MB/s (10^6)
            bytes_sec = (bytes_per_it * 1e9) / med_ns if med_ns > 0 else 0
            thru_mib_s = f"{bytes_sec / (1024 * 1024):.2f} MiB/s"
            thru_mb_s = f"{bytes_sec / 1_000_000:.2f} MB/s"

        lines.append(
            f"| **{wl}** | {op_label} | {input_label} | {med_ns:,} | {min_ns:,} | {max_ns:,} | {thru_rec_s} | {thru_mib_s} | {thru_mb_s} |"
        )

    micro_mixed = next((m for m in microbenchmarks if m.get("workload") == "mixed" and m.get("operation") == "decode"), None)
    micro_text = next((m for m in microbenchmarks if m.get("workload") == "text" and m.get("operation") == "decode"), None)
    mixed_rate_str = f"~{(micro_mixed['records_per_iteration'] * 1e9 / micro_mixed['median_ns_per_iteration']) / 1e6:.2f}M rec/s" if micro_mixed else "~4.5M rec/s"
    text_rate_str = f"~{(micro_text['records_per_iteration'] * 1e9 / micro_text['median_ns_per_iteration']) / 1e6:.2f}M rec/s" if micro_text else "~36M rec/s"

    lines.extend([
        "",
        "> [!IMPORTANT]",
        f"> **Microbenchmarks vs. End-to-End**: Microbenchmarks measure in-memory decoding throughput (up to {mixed_rate_str} on mixed records and {text_rate_str} on pure text). End-to-end benchmarks include file I/O, Arrow array allocation, Parquet encoding, dictionary creation, and disk flushes (yielding ~1.4M–1.5M rec/s for M3). These two classes of measurements address distinct engineering layers and must not be conflated.",
        "",
        "---",
        "",
        "## 6. Historical vs. Fresh Comparison",
        "",
        "Comparison with historical baseline observations on the same machine architecture (Ryzen 5 3400G / 16 GB DDR4). Deltas are calculated automatically from raw median durations:",
        "",
        "| Benchmark Scenario | Historical Observation | Fresh Canonical Measurement | Elapsed Time Delta | Throughput Delta / Notes |",
        "|---|---:|---:|---:|---|",
    ])

    for base in HISTORICAL_BASELINES:
        sc_name = base["scenario"]
        hist_label = base["hist_label"]
        hist_ms = base.get("hist_ms")
        hist_ns = base.get("hist_ns")

        # Find matching fresh benchmark
        fresh_label = "N/A"
        time_delta_str = "N/A"
        thru_delta_str = "N/A"

        if "benchmark" in base:
            match = next((b for b in benchmarks if b["benchmark"] == base["benchmark"] and b.get("records") == base.get("records") and b.get("batch_records") == base.get("batch")), None)
            if not match and "payload_bytes" in base:
                match = next((b for b in benchmarks if b["benchmark"] == base["benchmark"] and b.get("payload_bytes") == base.get("payload_bytes")), None)

            if match:
                runs = match["runs"]
                fresh_ms = compute_median([r["wall_clock_elapsed_ms"] for r in runs])
                peak_ws_mib = max(r.get("observed_peak_working_set_bytes") or 0 for r in runs) / (1024 * 1024)

                recs = match.get("records")
                if recs:
                    rec_s = (recs * 1000.0) / fresh_ms
                    fresh_label = f"{fresh_ms:,.2f} ms ({rec_s / 1_000_000:.2f}M rec/s)"
                    if base.get("hist_recs_s"):
                        thru_delta = ((rec_s - base["hist_recs_s"]) / base["hist_recs_s"]) * 100.0
                        thru_delta_str = f"{thru_delta:+.1f}% throughput (WS: {peak_ws_mib:.2f} MiB)"
                else:
                    # M5
                    payload_bytes = match.get("payload_bytes", 0)
                    size_mib = payload_bytes / (1024 * 1024)
                    mib_s = (size_mib * 1000.0) / fresh_ms if fresh_ms > 0 else 0
                    fresh_label = f"{fresh_ms:,.2f} ms ({mib_s:.2f} MiB/s)"
                    if base.get("hist_mib_s"):
                        thru_delta = ((mib_s - base["hist_mib_s"]) / base["hist_mib_s"]) * 100.0
                        thru_delta_str = f"{thru_delta:+.1f}% throughput (WS: {peak_ws_mib:.2f} MiB)"

                if hist_ms:
                    time_delta = ((fresh_ms - hist_ms) / hist_ms) * 100.0
                    time_delta_str = f"{time_delta:+.1f}%"
        elif "workload" in base:
            micro_match = next((m for m in microbenchmarks if m["workload"] == base["workload"] and m["operation"] == base["operation"]), None)
            if micro_match:
                fresh_ns = micro_match["median_ns_per_iteration"]
                rec_rate = (micro_match["records_per_iteration"] * 1e9) / fresh_ns
                fresh_label = f"{fresh_ns:,} ns/it ({rec_rate / 1_000_000:.2f}M rec/s)"
                if hist_ns:
                    time_delta = ((fresh_ns - hist_ns) / hist_ns) * 100.0
                    time_delta_str = f"{time_delta:+.1f}%"
                if base.get("hist_recs_s"):
                    thru_delta = ((rec_rate - base["hist_recs_s"]) / base["hist_recs_s"]) * 100.0
                    thru_delta_str = f"{thru_delta:+.1f}% throughput"

        lines.append(
            f"| {sc_name} | {hist_label} | {fresh_label} | {time_delta_str} | {thru_delta_str} |"
        )

    lines.extend([
        "",
        "---",
        "",
        "## 7. How to Reproduce",
        "",
        "### Running the Smoke Verification",
        "```powershell",
        "./scripts/benchmark.ps1 -Profile Smoke",
        "```",
        "",
        "### Running the Publication-Grade Full Suite",
        "```powershell",
        "./scripts/benchmark.ps1 -Profile Full -OutputJson docs/evidence/benchmark-full.json",
        "```",
        "",
        "### Regenerating and Checking Documentation from JSON",
        "```powershell",
        "# Regenerate BENCHMARKS.md deterministically from JSON evidence",
        "python scripts/render_benchmarks.py",
        "",
        "# Validate that documentation is strictly up-to-date with raw evidence (CI gate)",
        "python scripts/render_benchmarks.py --check",
        "```",
        "",
        "### Inspecting Raw JSON Output",
        "The raw output preserves exact timings, all individual runs, and environment hashes:",
        "```powershell",
        "Get-Content docs/evidence/benchmark-full.json | ConvertFrom-Json",
        "```",
        "",
    ])

    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Render or validate BENCHMARKS.md from benchmark JSON evidence."
    )
    parser.add_argument(
        "--json",
        default="docs/evidence/benchmark-full.json",
        help="Path to canonical benchmark JSON (default: docs/evidence/benchmark-full.json)",
    )
    parser.add_argument(
        "--output",
        default="docs/BENCHMARKS.md",
        help="Path to output BENCHMARKS.md markdown file (default: docs/BENCHMARKS.md)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Verify whether the existing output matches the derived content without writing.",
    )

    args = parser.parse_args()

    json_path = Path(args.json)
    if not json_path.exists():
        print(f"Error: Benchmark JSON file not found: {json_path}", file=sys.stderr)
        return 2

    try:
        with open(json_path, "r", encoding="utf-8") as f:
            data = json.load(f)
    except Exception as e:
        print(f"Error parsing JSON from {json_path}: {e}", file=sys.stderr)
        return 2

    out_path = Path(args.output)
    out_dir = str(out_path.parent).replace("\\", "/")

    try:
        rendered = render_markdown(
            data,
            json_rel_path=str(json_path).replace("\\", "/"),
            out_dir=out_dir,
        )
    except Exception as e:
        print(f"Error rendering markdown: {e}", file=sys.stderr)
        return 2

    if args.check:
        if not out_path.exists():
            print(f"FAIL: Output markdown file does not exist: {out_path}", file=sys.stderr)
            return 1

        with open(out_path, "r", encoding="utf-8") as f:
            existing = f.read()

        # Normalize newlines for cross-platform comparison
        existing_norm = existing.replace("\r\n", "\n")
        rendered_norm = rendered.replace("\r\n", "\n")

        if existing_norm != rendered_norm:
            print(f"FAIL: {out_path} is stale or differs from {json_path}!", file=sys.stderr)
            diff = difflib.unified_diff(
                existing_norm.splitlines(keepends=True),
                rendered_norm.splitlines(keepends=True),
                fromfile=str(out_path),
                tofile="rendered_from_json",
                n=3,
            )
            print("".join(list(diff)[:50]), file=sys.stderr)
            return 1
        else:
            print(f"PASS: {out_path} is strictly up-to-date with {json_path}.")
            return 0
    else:
        out_path.parent.mkdir(parents=True, exist_ok=True)
        with open(out_path, "w", encoding="utf-8", newline="\n") as f:
            f.write(rendered)
        print(f"Successfully generated {out_path} from {json_path}.")
        return 0


if __name__ == "__main__":
    sys.exit(main())
