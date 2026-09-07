const repository = "https://github.com/01110100chony/m2c-binary-engine"

// Change this single ref to `main` after the publication evidence is merged there.
export const repositoryRef = "m6-v.2-front-2"

export const projectLinks = {
  source: repository,
  architecture: `${repository}/blob/${repositoryRef}/docs/ARCHITECTURE.md`,
  benchmarks: `${repository}/blob/${repositoryRef}/docs/BENCHMARKS.md`,
  validation: `${repository}/blob/${repositoryRef}/docs/EXTERNAL_COMPATIBILITY.md`,
  copybookSubset: `${repository}/blob/${repositoryRef}/docs/COPYBOOK_SUBSET.md`,
  evidence: `${repository}/blob/${repositoryRef}/docs/M6_EVIDENCE.md`,
  fixture: `${repository}/tree/${repositoryRef}/tests/fixtures`,
} as const

export const nav = [
  { id: "about", label: "About" },
  { id: "demo", label: "Demo" },
  { id: "architecture", label: "Architecture" },
  { id: "benchmarks", label: "Benchmarks" },
  { id: "validation", label: "Validation" },
] as const

export const learningTopics = [
  "Rust",
  "Binary parsing",
  "EBCDIC",
  "DISPLAY · COMP · COMP-3",
  "Apache Arrow",
  "Apache Parquet",
  "Recovery",
  "Benchmark design",
  "Post-quantum cryptography",
] as const

export const architectureStages = [
  { label: "Compiled typed layout", note: "Offsets, byte lengths, encodings, precision, and scale" },
  { label: "Batch decode", note: "Supported EBCDIC and numeric representations" },
  { label: "Apache Arrow", note: "Typed in-memory record batches" },
  { label: "Apache Parquet", note: "Incremental local columnar output" },
] as const
