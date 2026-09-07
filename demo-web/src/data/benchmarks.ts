/**
 * Canonical publication values from docs/BENCHMARKS.md and
 * docs/evidence/benchmark-full.json on repositoryRef `m6-v.2-front-2`.
 */
export const publishedBenchmarks = {
  context: "Local synthetic benchmark · 3,000,000 records · 100.14 MiB source input",
  primary: {
    records: "3,000,000",
    medianMs: 2096.45,
    recordsPerSecond: 1_430_990,
    workingSetMiB: 15.26,
    batch: 65_536,
  },
  conversion: [
    { batch: 256, medianMs: 12_471.72, recordsPerSecond: 240_544, workingSetMiB: 132.18 },
    { batch: 4_096, medianMs: 2_148.79, recordsPerSecond: 1_396_136, workingSetMiB: 13.93 },
    { batch: 65_536, medianMs: 2_096.45, recordsPerSecond: 1_430_990, workingSetMiB: 15.26 },
  ],
  recovery: [
    { batch: 4_096, parts: 733, medianMs: 291_058.86, recordsPerSecond: 10_307, sourceMiBPerSecond: 0.34, workingSetMiB: 6.88 },
    { batch: 65_536, parts: 46, medianMs: 5_049.99, recordsPerSecond: 594_061, sourceMiBPerSecond: 19.83, workingSetMiB: 15.24 },
  ],
  mixedDecoder: {
    recordsPerIteration: 768,
    medianNs: 168_367,
    recordsPerSecond: 4_561_464,
    sourceMiBPerSecond: 152.26,
  },
  protection: [
    { operation: "Protect", medianMs: 5_466.58, sourceMiBPerSecond: 11.71, workingSetMiB: 5.27 },
    { operation: "Unprotect", medianMs: 877.25, sourceMiBPerSecond: 72.95, workingSetMiB: 5.27 },
  ],
} as const

export const benchmarkTabs = [
  { id: "conversion", label: "Conversion" },
  { id: "recovery", label: "Recovery" },
  { id: "details", label: "Technical details" },
] as const

export const benchmarkDisclaimer =
  "Single-workstation measurements over deterministic synthetic data. They are not an SLA or a universal memory bound. Working-set values are the greatest PeakWorkingSet64 samples observed by the Windows harness, not exact heap or RSS peaks."
