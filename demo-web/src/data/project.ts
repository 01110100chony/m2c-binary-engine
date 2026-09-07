export type Status = "PASS" | "WARN" | "FAIL"

const repository = "https://github.com/01110100chony/m2c-binary-engine"

export const projectLinks = {
  source: repository,
  architecture: `${repository}/blob/m6-candidate/docs/ARCHITECTURE.md`,
  benchmarks: `${repository}/blob/m6-candidate/docs/M6_RESULTS.md`,
  evidence: `${repository}/blob/m6-candidate/docs/M6_EVIDENCE.md`,
} as const

export const nav = [
  { id: "overview", label: "Overview" },
  { id: "pipeline", label: "Pipeline" },
  { id: "benchmarks", label: "Benchmarks" },
  { id: "reliability", label: "Reliability" },
  { id: "security", label: "Security" },
  { id: "milestones", label: "Milestones" },
] as const

export const pipelineStages = [
  { key: "copybook", label: "COBOL Copybook" },
  { key: "layout", label: "Compiled Layout" },
  { key: "decode", label: "Binary Decode" },
  { key: "arrow", label: "Arrow" },
  { key: "parquet", label: "Parquet" },
  { key: "recovery", label: "Recovery", optional: true },
  { key: "pqc", label: "PQC Protection", optional: true },
] as const

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
] as const

export const recoveryProperties = [
  "Deterministic parts",
  "Immutable receipts",
  "No overwrite",
  "Resume validation",
  "Independent integrity checking",
] as const

export const pqcFlow = [
  { label: "Parquet Artifact", note: "Read-only input" },
  { label: "ML-KEM-768", note: "Key establishment" },
  { label: "HKDF-SHA-256", note: "Key derivation" },
  { label: "AES-256-GCM", note: "Authenticated encryption" },
  { label: "Protected Artifact", note: "Separate output" },
] as const

export const pqcProperties = [
  { label: "Optional Cargo feature", value: "pqc" },
  { label: "Streaming chunks", value: "1 MiB" },
  { label: "Authenticated encryption", value: "AES-256-GCM" },
  { label: "Publication", value: "No-clobber" },
] as const

export const evidence: { label: string; scope?: string; status: Status }[] = [
  { label: "Formatting", status: "PASS" },
  { label: "Clippy", status: "PASS" },
  { label: "Tests", status: "PASS" },
  { label: "Doctests", status: "PASS" },
  { label: "Generative campaigns", scope: "Full local", status: "PASS" },
  { label: "Mutation campaigns", scope: "Full local", status: "PASS" },
  { label: "External Parquet verification", status: "PASS" },
  { label: "Benchmarks", scope: "Full local", status: "PASS" },
  { label: "CI Smoke", scope: "Remote Smoke only", status: "PASS" },
]

export const milestones: { id: string; title: string; status: Status }[] = [
  { id: "M0", title: "Foundation", status: "PASS" },
  { id: "M1", title: "Copybook Compiler", status: "PASS" },
  { id: "M2", title: "Binary → Arrow", status: "PASS" },
  { id: "M3", title: "Parquet Pipeline", status: "PASS" },
  { id: "M4", title: "Recovery / Resume", status: "PASS" },
  { id: "M5", title: "PQC Protection", status: "PASS" },
  { id: "M6", title: "Evidence / Benchmarks", status: "PASS" },
]
