import type { Status } from "@/data/project"

/** Presentation-only values shaped for future replacement by CLI --report-json output. */
export const simulatedDemo = {
  copybook: `01 CUSTOMER-RECORD.
   05 CUSTOMER-ID   PIC 9(6).
   05 NAME          PIC X(20).
   05 BALANCE       PIC S9(7)V99 COMP-3.`,
  stages: [
    { key: "compile", label: "Compile Copybook" },
    { key: "decode", label: "Decode Records" },
    { key: "arrow", label: "Build Arrow Batch" },
    { key: "parquet", label: "Write Parquet Parts" },
    { key: "verify", label: "Verify Output" },
  ],
  report: {
    status: "PASS" as Status,
    records: 300_000,
    parts: 5,
    format: "Parquet",
    verification: "PASS" as Status,
  },
  terminalLines: [
    { text: "$ m2c convert-parts --copybook customer.cpy --input data.bin \\", type: "cmd" },
    { text: "    --output-dir output/ --batch-records 65536 --report-json", type: "cmd" },
    { text: "copybook compiled", type: "ok" },
    { text: "records decoded", type: "ok" },
    { text: "parquet parts written", type: "ok" },
    { text: "output verified", type: "ok" },
  ],
} as const

export type DemoStageState = "waiting" | "running" | "complete"
export type TerminalLine = (typeof simulatedDemo.terminalLines)[number]
