# M2C Binary ETL Engine

**Mainframe-to-Cloud Binary ETL Engine with Post-Quantum Security and AIOps Integration**

> A systems engineering project built in Rust, tackling one of the most critical and underexplored challenges in modern financial infrastructure: moving massive volumes of binary Mainframe data to the cloud, in real time, with integrity, and with security architecture designed for the next decade of threats.

---

## Project Status

This is an active portfolio project. Core binary parsing, endianness handling, and PQC integration are implemented. AIOps telemetry pipeline and cloud sink connectors are in progress.

Contributions, issues, and technical feedback are welcome.

---

## The Problem Worth Solving

Most financial institutions still run their core operations on IBM z/OS Mainframes. Not out of inertia, but because these systems genuinely deliver a level of reliability and transaction throughput that modern platforms have yet to replicate at scale. The Mainframe is not going away. What is changing is the expectation around what banks can do with that data, and how fast .

The gap between Mainframe-generated data and cloud-based analytics is wide, expensive, and increasingly dangerous to ignore. Three forces are converging to make this problem urgent:

**Processing cost.** EBCDIC-to-ASCII translation, COMP-3 packed decimal decoding, and copybook parsing are routinely performed on the Mainframe itself — consuming MIPS that are billed at significant cost. Every CPU cycle that can be offloaded represents direct financial savings.

**Encryption vulnerability.** RSA and ECC, the cryptographic standards protecting financial data in transit today, are known to be theoretically vulnerable to sufficiently powerful quantum computers. The threat is not immediate, but the attack vector is real: adversaries are already harvesting encrypted traffic today with the intent to decrypt it once quantum capability matures. "Harvest Now, Decrypt Later" is not a hypothetical , it is an active threat model documented by intelligence agencies and financial regulators.

**Operational opacity.** Hybrid pipelines between Mainframe and cloud environments are notoriously difficult to monitor. When something degrades or breaks, the mean time to detection is high, and the mean time to root cause is higher. Predictive observability in this layer is essentially nonexistent in most institutions.

This project was built to confront all three.

---

## What This Engine Does

The M2C Binary ETL Engine is a high-performance data pipeline written in Rust, designed to extract binary data from z/OS Mainframe environments, transform it in-flight, and deliver it to cloud storage with quantum-resistant encryption — while exporting rich telemetry for AI-driven operational monitoring.

It is not a proof-of-concept wrapper around existing tooling. The transformation logic, binary parsing, and security layer are implemented from the ground up, with the performance and correctness guarantees that Rust's memory model enables.

---

## Architecture

### Ingestion Layer

Data enters the engine through high-speed connectors or local agents deployed at the Mainframe boundary. The interface is designed to handle the specific quirks of z/OS data: variable-length records, multi-segment copybooks, and the endianness mismatch between Big-Endian Mainframe and Little-Endian cloud targets.

### Transformation Core (Rust)

This is where the majority of the engineering work lives.

Binary parsing is performed with zero-copy reads where possible, avoiding the allocation overhead that makes naive implementations fall apart at scale. EBCDIC-to-UTF-8 translation, COMP-3 packed decimal decoding, and copybook-driven schema inference are handled natively, producing structured output in columnar formats — specifically Apache Parquet — ready for direct consumption by analytical engines.

Endianness conversion is handled deterministically, with explicit handling for the Big-Endian-to-Little-Endian boundary that causes silent data corruption in less careful implementations.

### Post-Quantum Security Layer

Encryption is not bolted on at the end of the pipeline — it is part of the transformation step itself.

The engine integrates [liboqs](https://github.com/open-quantum-safe/liboqs) to implement **ML-KEM (Kyber)**, one of the algorithms standardized by NIST in its Post-Quantum Cryptography project (FIPS 203). Data is encrypted before it leaves the transformation core, meaning it arrives at the cloud landing zone already protected against both classical and quantum adversaries.

This design choice directly addresses the "Harvest Now, Decrypt Later" threat vector — a concern that is increasingly appearing in financial regulatory guidance and internal security reviews at major institutions.

### Data Sink

Structured, encrypted data lands in Azure Blob Storage or Azure Data Lake, partitioned and formatted for immediate analytical access. The schema is preserved from the copybook definition, meaning downstream consumers do not need knowledge of the original Mainframe data layout.

### Monitoring and AIOps Integration

The engine emits detailed telemetry at every stage of the pipeline: ingestion throughput, transformation latency, encryption overhead, and sink write performance. These metrics are exported via Prometheus and visualized in Grafana.

Beyond passive monitoring, the telemetry feed is designed for integration with AI-driven anomaly detection. Unusual patterns in binary data streams — unexpected record length distributions, throughput drops, encoding anomalies — can indicate hardware degradation on the Mainframe side or, more critically, data exfiltration attempts. AIOps agents consuming this feed can surface predictive signals before they manifest as incidents.

---

## Technical Stack

| Layer | Technology |
|---|---|
| Core Engine | Rust (stable) |
| Binary Parsing | Custom copybook parser — EBCDIC, COMP-3, packed decimals |
| Output Format | Apache Parquet |
| Quantum-Safe Cryptography | liboqs — ML-KEM / Kyber (NIST FIPS 203) |
| Cloud Target | Azure Blob Storage / Azure Data Lake Gen2 |
| Observability | Prometheus + Grafana |
| AIOps Integration | Prometheus remote-write to AI monitoring pipeline |

---

## Strategic Context

The intersection of Mainframe modernization, quantum-readiness, and AIOps is not a niche corner of the industry; it is where some of the most significant engineering investment in global banking is currently concentrated. Regulators in multiple jurisdictions have begun issuing guidance on quantum risk in financial services. Cloud migration from core banking systems is accelerating. And the operational complexity of hybrid environments is pushing institutions toward AI-driven infrastructure management. Eventually, it is the most efficient step we should have to modernize the financial system.

This project was built at that intersection, not as a theoretical exercise, but as a working implementation of what that future looks like in code.

## References

- [NIST FIPS 203 — ML-KEM Standard](https://csrc.nist.gov/pubs/fips/203/final)
- [Open Quantum Safe — liboqs](https://openquantumsafe.org/)
- [Apache Parquet Format Specification](https://parquet.apache.org/docs/file-format/)
- [IBM z/OS Data Formats — EBCDIC and COMP-3](https://www.ibm.com/docs/en/zos)
- [Azure Data Lake Storage Gen2](https://learn.microsoft.com/en-us/azure/storage/blobs/data-lake-storage-introduction)
