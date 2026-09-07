# M2C External Differential Validation — Spark / Cobrix 2.9.4

Status: Differential correctness verification (not a performance benchmark).  
Reference artifact: `scratch/external-cobol-test/`  
Validation date: 2026-09-06

---

## 1. Purpose and Scope

This document records independent external differential correctness evidence for M2C. The goal is to demonstrate that M2C decodes externally generated, mainframe-formatted binary records (CP037 EBCDIC text combined with IBM-style packed decimals) into typed columnar data that semantically matches an industry-standard mainframe connector: **AbsaOSS Cobrix** on **Apache Spark**.

This test provides correctness evidence only. It is **not** a benchmark comparing throughput or latency between M2C and Spark/Cobrix.

---

## 2. External Origin and Generation Pipeline

- **Upstream source**: [ravi-asati/data-engineering-notes](https://github.com/ravi-asati/data-engineering-notes)
- **Article / Reference**: `articles/2025-12-mainframe-for-data-engineers/cobol-to-parquet`
- **Pinned upstream commit / tree**: `da33252e98f47a836ba995e43de327b1d7034f3c` (branch `main`)

### Generation Flow
1. **Record synthesis**: Synthesized transaction records produced via GnuCOBOL fixed-format program.
2. **Encoding step**: Text fields converted from ASCII to IBM CP037 EBCDIC byte values; COMP-3 packed decimals preserved in IBM-standard nibble format.
3. **Output artifact**: `TRXN_COBOL_DATA.ebcdic` (named `input.ebcdic` in repository tests).

### Validated Dataset Properties
- **Total records**: 100
- **Record length**: 24 bytes
- **Total file size**: 2,400 bytes
- **SHA-256 (`input.ebcdic`)**: `a292e8aa6317a247e2fa6091d054449914f21140a46ecfa83d02dff6b0098083`

---

## 3. Copybook Specification and Syntax Adaptation

### Original External Copybook
```cobol
       01 TRXN-REC.
          05 TRXN-ID     PIC 9(9)       COMP-3.
          05 TRXN-DT     PIC X(8).
          05 TRXN-TM     PIC X(6).
          05 TRXN-AMNT   PIC S9(7)V99   COMP-3.
```

### Physical Record Layout (24 bytes)
| Field | Offset | Length | COBOL Clause | Physical Representation |
|---|---|---|---|---|
| `TRXN-ID` | 0 | 5 bytes | `PIC 9(9) COMP-3` | 9 digits packed into 5 bytes (unsigned nibbles, terminal sign nibble `F`) |
| `TRXN-DT` | 5 | 8 bytes | `PIC X(8)` | CP037 EBCDIC string (YYYYMMDD) |
| `TRXN-TM` | 13 | 6 bytes | `PIC X(6)` | CP037 EBCDIC string (HHMMSS) |
| `TRXN-AMNT` | 19 | 5 bytes | `PIC S9(7)V99 COMP-3` | 9 digits (7 integer + 2 fractional), packed into 5 bytes with signed nibble (`C`/`D`) |

### M2C Syntax Adaptation
M2C v0.1 accepts equivalent supported explicit syntax in `layout.cpy`:
```cobol
       01 TRXN-REC.
          05 TRXN-ID     PIC 9(9) COMP-3.
          05 TRXN-DT     PIC X(8).
          05 TRXN-TM     PIC X(6).
          05 TRXN-AMNT   PIC S9(7)V9(2) COMP-3.
```

> [!NOTE]
> `V99` and `V9(2)` describe identical physical and logical semantics: a 2-digit implicit decimal fraction. M2C's frozen copybook grammar requires the parenthesized count format `V9(m)`. This is a syntactic normalization for M2C v0.1, not a semantic modification.

---

## 4. Environment and Decode Configurations

### 4.1 M2C Pipeline (Rust)
- **Engine**: `m2c-pipeline` 0.1.0 (release build, `parquet` 53, `arrow` 53)
- **Command**:
  ```powershell
  cargo run --release --bin m2c-pipeline -- convert `
    --copybook scratch/external-cobol-test/layout.cpy `
    --input scratch/external-cobol-test/input.ebcdic `
    --output scratch/external-cobol-test/output.parquet `
    --batch-records 100
  ```
- **Observed Arrow / Parquet Schema**:
  - `TRXN-REC.TRXN-ID`: `decimal128(9, 0)` (non-null)
  - `TRXN-REC.TRXN-DT`: `string` (non-null)
  - `TRXN-REC.TRXN-TM`: `string` (non-null)
  - `TRXN-REC.TRXN-AMNT`: `decimal128(9, 2)` (non-null)
- **Determinism**: The output file has SHA-256 `b3190a915e5ec5ea97391edf702558f521970c217c0acbbd8313730c49acfaf8` (3,926 bytes).

### 4.2 Apache Spark + Cobrix Environment
- **Platform**: Ubuntu 24.04 LTS on WSL2 (Kernel 5.15.x / 6.x)
- **Java Runtime**: OpenJDK 17.0.20
- **Apache Spark**: Spark 4.0.1 (Scala 2.13.x)
- **Cobrix Connector**: `za.co.absa.cobrix:spark-cobol_2.13:2.9.4`
- **Cobrix Configuration**:
  ```python
  df = spark.read.format("cobol") \
      .option("copybook", "/tmp/copybook.cpy") \
      .option("record_format", "F") \
      .option("record_length", "24") \
      .load("input.ebcdic")
  df.write.mode("overwrite").parquet("cobrix_trxn_parquet")
  ```
- **Observed Spark Schema**:
  - `TRXN_ID`: `integer` (`int32`, nullable)
  - `TRXN_DT`: `string` (nullable)
  - `TRXN_TM`: `string` (nullable)
  - `TRXN_AMNT`: `decimal(9, 2)` (nullable)

---

## 5. Differential Comparison Methodology

Raw Parquet binary files are **not** expected to match byte-for-byte between independent writers. Parquet writers differ in:
- Writer metadata and version strings (`parquet-mr` vs `arrow-rs`);
- Compression settings (Snappy vs uncompressed);
- Row group boundaries and page layout;
- Dictionary encoding choices;
- Metadata ordering and statistics headers.

### Semantic Equality Criteria
Correctness is defined as **semantic equivalence** after standard, well-defined normalizations:
1. **Column Name Normalization**: Strip root/group path prefix (`TRXN-REC.` -> `TRXN-ID`) and convert hyphens to underscores (`TRXN_ID`).
2. **Numeric Representation Normalization**:
   - `TRXN_ID`: Cobrix represents unscaled COMP-3 with precision 9 as `int32`, while M2C strictly preserves decimal semantics as `Decimal128(9, 0)`. Both represent integers in range `[0, 999,999,999]`. Equality is evaluated over integer values (`int(m2c_val) == int(cobrix_val)`). No floating-point operations are used.
   - `TRXN_AMNT`: Both engines emit `Decimal(9, 2)`. Decimal values are verified with exact mathematical scale and precision.
3. **String Fields**: Exact string equality after CP037 decode (`TRXN_DT` format `YYYYMMDD`, `TRXN_TM` format `HHMMSS`).
4. **Key Uniqueness & Ordering**:
   - Verified that `TRXN_ID` has zero duplicate values in M2C and zero in Cobrix.
   - Verified that the key sequence is identical without re-indexing.

---

## 6. Execution Results

Validation was performed using `scripts/compare_cobrix.py`:

```bash
python scripts/compare_cobrix.py \
  --m2c scratch/external-cobol-test/output.parquet \
  --cobrix scratch/external-cobol-test/cobrix_trxn_parquet
```

### Comparator Output
```text
============================================================
M2C vs Spark/Cobrix Parquet Differential Report
============================================================
M2C original columns:    ['TRXN-REC.TRXN-ID', 'TRXN-REC.TRXN-DT', 'TRXN-REC.TRXN-TM', 'TRXN-REC.TRXN-AMNT']
Cobrix original columns: ['TRXN_ID', 'TRXN_DT', 'TRXN_TM', 'TRXN_AMNT']
Normalized columns:      ['TRXN_AMNT', 'TRXN_DT', 'TRXN_ID', 'TRXN_TM']
Row count: M2C = 100, Cobrix = 100
Key ordering: IDENTICAL across all 100 records.
Records compared: 100
Field comparison: ALL 4 fields matched across all records.
============================================================
STATUS: PASS (100% semantically identical records)
```

Sample records decoded by both engines:
| `TRXN_ID` | `TRXN_DT` | `TRXN_TM` | `TRXN_AMNT` | Match Status |
|---|---|---|---|---|
| `324638715` | `20260828` | `054238` | `-1575.46` | EXACT |
| `324638716` | `20260905` | `065609` | `4662.84` | EXACT |
| `324638717` | `20260809` | `034928` | `1379.66` | EXACT |
| `324638718` | `20260817` | `041714` | `-665.51` | EXACT |
| `324638719` | `20260905` | `130535` | `2672.42` | EXACT |
| ... (95 more) | ... | ... | ... | EXACT |

---

## 7. Scope Boundaries and Safe Technical Claim

> [!IMPORTANT]
> This dataset is an externally generated, realistic mainframe-style transaction dataset. It is **not** a production-extracted mainframe dataset, nor does it prove universal COBOL support (e.g. `OCCURS`, `REDEFINES`, or variable-length records remain explicitly unsupported in v0.1).

### Supported Technical Claim
> "M2C successfully processed an externally generated GnuCOBOL fixed-record dataset using CP037 text and COMP-3 numerics. An independent Spark/Cobrix 2.9.4 decode of the same 2,400-byte input produced 100/100 semantically identical logical records after schema normalization."
