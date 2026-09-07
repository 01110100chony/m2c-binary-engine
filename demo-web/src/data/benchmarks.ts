import type { Status } from "@/data/project"

/** Documented local synthetic measurements from docs/M6_RESULTS.md. */
export const measuredEvidence = {
  heroMetrics: [
    { value: "3,000,000", label: "Records benchmarked", sub: "Synthetic fixed-record dataset" },
    { value: "2.279 s", label: "Median M3 runtime", sub: "3M records / batch 65,536" },
    { value: "15.39 MiB", label: "Observed working set", sub: "3M records / batch 65,536" },
    { value: "PASS", label: "Independent verification", sub: "External Parquet reader", status: "PASS" as Status },
  ],
  m3Runtime: {
    title: "Median runtime by batch size",
    dataset: "3,000,000 records",
    unit: "ms",
    bars: [
      { label: "Batch 256", value: 3401.83 },
      { label: "Batch 4,096", value: 2524.8 },
      { label: "Batch 65,536", value: 2279.18 },
    ],
  },
  m3WorkingSet: [
    { label: "256", value: "131.69 MiB" },
    { label: "4,096", value: "13.88 MiB" },
    { label: "65,536", value: "15.39 MiB" },
  ],
  m4Create: {
    title: "Create — median runtime",
    dataset: "300,000 records",
    unit: "ms",
    bars: [
      { label: "Batch 256", value: 27092.16 },
      { label: "Batch 4,096", value: 2129.12 },
      { label: "Batch 65,536", value: 430.46 },
    ],
  },
  m4Resume: {
    title: "Resume validation — median runtime",
    dataset: "300,000 records",
    unit: "ms",
    bars: [
      { label: "Batch 256", value: 938.72 },
      { label: "Batch 4,096", value: 210.09 },
      { label: "Batch 65,536", value: 117.63 },
    ],
  },
  m4Scale: [
    { operation: "Create", records: "3,000,000", batch: "65,536", median: "4,163.58 ms", workingSet: "15.29 MiB" },
    { operation: "Resume validation", records: "3,000,000", batch: "65,536", median: "234.74 ms", workingSet: "7.05 MiB" },
  ],
  m5: {
    title: "64 MiB artifact — median runtime",
    dataset: "Optional pqc feature; operations measured separately",
    unit: "ms",
    bars: [
      { label: "Protect", value: 2939.66 },
      { label: "Unprotect", value: 1182.56 },
    ],
  },
  micro: [
    { fixture: "Mixed", compile: "21,148", decode: "171,722", records: "768" },
    { fixture: "Text", compile: "3,529", decode: "7,114", records: "256" },
    { fixture: "Numeric", compile: "10,791", decode: "18,623", records: "256" },
  ],
} as const

export const benchmarkTabs = [
  { id: "m3", label: "M3 Conversion" },
  { id: "m4", label: "M4 Recovery" },
  { id: "m5", label: "M5 Protection" },
  { id: "micro", label: "Microbench" },
] as const

export const benchmarkDisclaimer =
  "Synthetic fixed-record benchmark. Local measurements. Not an SLA or universal memory bound. Working set is the greatest PeakWorkingSet64 value observed during local execution, not exact heap usage or exact peak RSS."
