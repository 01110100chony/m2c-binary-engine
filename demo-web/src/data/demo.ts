/**
 * Verified reference data from tests/fixtures/sample_fixed.{cpy,bin}.
 * The Parquet artifact was generated with the exact command below using the real M2C CLI.
 */
export const referenceDemo = {
  fixture: "tests/fixtures/sample_fixed.{cpy,bin}",
  copybook: `000100 01 SAMPLE-RECORD.
000200 05 HEADER-GROUP.
000300* THIS FIXED-FORMAT COMMENT MUST BE IGNORED.
000400 10 CUSTOMER-NAME PIC X(10).
000500 10 FILLER PIC X(2).
000600 10 ACCOUNT-NUMBER PIC 9(4) DISPLAY.
000700 10 INTEREST-RATE PIC 9(5)V9(2) DISPLAY.
000800 10 BALANCE-BIN PIC S9(4) COMP.
000900 10 RATE-BIN PIC 9(5)V9(2) BINARY.
001000 10 AMOUNT-PACKED PIC S9(7)V9(2) COMP-3.
001100 10 FILLER PIC X.`,
  hexPreview:
    "C1 D3 C9 C3 C5 40 40 40 40 40 00 FF F0 F0 F4 F2 F0 F0 F1 F2 F3 F4 F5 FF 85 00 01 E2 40 12 34 56 78 9C AA",
  stages: ["Compile Copybook", "Decode fixed records", "Build Arrow batches", "Write Parquet"],
  command:
    "cargo run --release -- convert --report-json --copybook tests/fixtures/sample_fixed.cpy --input tests/fixtures/sample_fixed.bin --output sample-fixed.parquet --batch-records 2",
  report: {
    status: "success",
    records: 3,
    recordLength: 35,
    inputBytes: 105,
    rowGroups: 2,
    outputBytes: 3925,
    format: "Apache Parquet",
    sha256: "66758da7e50124b7d46756b5f9d6d83d338c9e68e67e9a3ee9320b35d634a400",
  },
  schema: [
    { field: "CUSTOMER-NAME", type: "Utf8" },
    { field: "ACCOUNT-NUMBER", type: "Int64" },
    { field: "INTEREST-RATE", type: "Decimal128(7, 2)" },
    { field: "BALANCE-BIN", type: "Int64" },
    { field: "RATE-BIN", type: "Decimal128(7, 2)" },
    { field: "AMOUNT-PACKED", type: "Decimal128(9, 2)" },
  ],
  rows: [
    { name: "ALICE·····", account: "42", interest: "123.45", balance: "−123", rate: "1,234.56", amount: "1,234,567.89" },
    { name: "José······", account: "9,999", interest: "99,999.99", balance: "9,999", rate: "99,999.99", amount: "−1.23" },
    { name: "\\0\\u{85}\\n¤[]····", account: "0", interest: "0.00", balance: "0", rate: "0.00", amount: "0.00" },
  ],
} as const
