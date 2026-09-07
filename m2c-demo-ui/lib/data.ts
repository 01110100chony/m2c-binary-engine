/**
 * Centralized static data for the M2C Binary Engine presentation frontend.
 *
 * Everything here is mock/demo data shaped to mirror the M2C CLI `--report-json`
 * output. Swapping these objects for real API responses later should not require
 * touching the components that render them.
 */

export type Status = "PASS" | "WARN" | "FAIL"

export const GITHUB_URL = "https://github.com"

export const nav = [
  { id: "overview", label: "Overview" },
  { id: "pipeline", label: "Pipeline" },
  { id: "benchmarks", label: "Benchmarks" },
  { id: "reliability", label: "Reliability" },
  { id: "security", label: "Security" },
  { id: "milestones", label: "Milestones" },
] as const

export const pipelineStages = [
  { key: "cobol", label: "COBOL" },
  { key: "binary", label: "Binary" },
  { key: "decode", label: "Decode" },
  { key: "arrow", label: "Arrow" },
  { key: "parquet", label: "Parquet" },
  { key: "recovery", label: "Recovery" },
  { key: "pqc", label: "PQC" },
] as const

export const metrics = [
  {
    value: "3,000,000",
    label: "Records benchmarked",
    sub: "Synthetic fixed-record dataset",
  },
  {
    value: "2.279 s",
    label: "Median M3 runtime",
    sub: "3M records / batch 65,536",
  },
  {
    value: "15.39 MiB",
    label: "Observed working set",
    sub: "3M records / batch 65,536",
  },
  {
    value: "PASS",
    label: "Independent verification",
    sub: "External Parquet reader",
    status: "PASS" as Status,
  },
]

export const copybookExample = `01 CUSTOMER-RECORD.
   05 CUSTOMER-ID   PIC 9(6).
   05 NAME          PIC X(20).
   05 BALANCE       PIC S9(7)V99 COMP-3.`

export type DemoStageState = "waiting" | "running" | "complete"

export const demoStages = [
  { key: "compile", label: "Compile Copybook" },
  { key: "decode", label: "Decode Records" },
  { key: "arrow", label: "Build Arrow Batch" },
  { key: "parquet", label: "Write Parquet" },
  { key: "verify", label: "Verify Output" },
] as const

/** Shaped like a future `m2c convert-parts --report-json` payload. */
export const demoReport = {
  status: "PASS" as Status,
  records: 300_000,
  parts: 5,
  format: "Parquet",
  verification: "PASS" as Status,
}

export const terminalLines = [
  { text: "$ m2c convert-parts --copybook customer.cpy --in data.bin \\", type: "cmd" },
  { text: "    --out output/ --batch 65536 --report-json", type: "cmd" },
  { text: "copybook compiled", type: "ok" },
  { text: "records decoded", type: "ok" },
  { text: "parquet written", type: "ok" },
  { text: "output verified", type: "ok" },
] as const

export type TerminalLine = (typeof terminalLines)[number]

/* ---------------------------------- benchmarks --------------------------------- */

export const benchmarkTabs = [
  { id: "m3", label: "M3 Conversion" },
  { id: "m4", label: "M4 Recovery" },
  { id: "m5", label: "M5 Protection" },
  { id: "micro", label: "Microbench" },
] as const

export const m3Runtime = {
  title: "Runtime by Batch Size",
  dataset: "3,000,000 records",
  unit: "ms",
  bars: [
    { label: "Batch 256", value: 3401.83 },
    { label: "Batch 4096", value: 2524.8 },
    { label: "Batch 65536", value: 2279.18 },
  ],
}

export const m3WorkingSet = {
  title: "Observed Working Set",
  metric: "Observed PeakWorkingSet64",
  rows: [
    { label: "256", value: "131.69 MiB" },
    { label: "4096", value: "13.88 MiB" },
    { label: "65536", value: "15.39 MiB" },
  ],
}

export const benchmarkDisclaimer =
  "Synthetic fixed-record benchmark. Local measurements. Not an SLA or universal memory bound."

export const m4Bench = {
  bars: [
    { label: "Cold run", value: 2279.18 },
    { label: "Resume (2/5 parts)", value: 1402.6 },
    { label: "Resume (4/5 parts)", value: 588.9 },
  ],
  unit: "ms",
  dataset: "3,000,000 records — resume from committed prefix",
}

export const m5Bench = {
  bars: [
    { label: "Plain write", value: 2279.18 },
    { label: "Protect (pqc)", value: 2611.44 },
    { label: "Verify + decrypt", value: 2740.02 },
  ],
  unit: "ms",
  dataset: "3M records — optional `pqc` feature enabled",
}

export const microBench = {
  bars: [
    { label: "COMP-3 decode", value: 41.2 },
    { label: "PIC X copy", value: 12.8 },
    { label: "Arrow append", value: 63.5 },
    { label: "Parquet encode", value: 118.7 },
  ],
  unit: "ns/record",
  dataset: "Isolated hot-path microbenchmarks",
}

/* ----------------------------------- recovery ---------------------------------- */

export const outputTree = `output/
├── manifest.json
├── parts/
│   ├── part-00000.parquet
│   ├── part-00001.parquet
│   └── part-00002.parquet
├── commits/
│   └── ...
└── complete.json`

export const recoverySteps = [
  "Process interrupted",
  "Resume",
  "Validate committed prefix",
  "Continue from next part",
  "Complete",
]

export const recoveryProperties = [
  "Deterministic parts",
  "Immutable receipts",
  "No overwrite",
  "Resume validation",
  "Independent integrity checking",
]

/* ----------------------------------- security ---------------------------------- */

export const pqcFlow = [
  { label: "Parquet Artifact", note: "Analytical output" },
  { label: "ML-KEM-768", note: "Key encapsulation" },
  { label: "HKDF-SHA-256", note: "Key derivation" },
  { label: "AES-256-GCM", note: "Authenticated encryption" },
  { label: "Protected Artifact", note: "Sealed output" },
]

export const pqcProperties = [
  { label: "Optional Cargo feature", value: "pqc" },
  { label: "Streaming chunks", value: "1 MiB" },
  { label: "Encryption", value: "Authenticated (AEAD)" },
  { label: "Publication", value: "No-clobber" },
]

/* ----------------------------------- evidence ---------------------------------- */

export const evidence: { label: string; status: Status }[] = [
  { label: "Formatting", status: "PASS" },
  { label: "Clippy", status: "PASS" },
  { label: "Tests", status: "PASS" },
  { label: "Doctests", status: "PASS" },
  { label: "Generative campaigns", status: "PASS" },
  { label: "Mutation campaigns", status: "PASS" },
  { label: "External Parquet verification", status: "PASS" },
  { label: "Benchmarks", status: "PASS" },
  { label: "CI Smoke", status: "PASS" },
]

/* ---------------------------------- milestones --------------------------------- */

export const milestones: { id: string; title: string; status: Status }[] = [
  { id: "M0", title: "Foundation", status: "PASS" },
  { id: "M1", title: "Copybook Compiler", status: "PASS" },
  { id: "M2", title: "Binary → Arrow", status: "PASS" },
  { id: "M3", title: "Parquet Pipeline", status: "PASS" },
  { id: "M4", title: "Recovery / Resume", status: "PASS" },
  { id: "M5", title: "PQC Protection", status: "PASS" },
  { id: "M6", title: "Evidence / Benchmarks", status: "PASS" },
]
