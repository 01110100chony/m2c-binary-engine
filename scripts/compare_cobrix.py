#!/usr/bin/env python3
"""Differential comparator: M2C Parquet vs. Apache Spark / Cobrix Parquet.

Verifies semantic equality between M2C-generated Parquet and Spark/Cobrix-generated
Parquet for legacy mainframe datasets (e.g. CP037 EBCDIC text + COMP-3 packed decimals).

Usage:
    python scripts/compare_cobrix.py --m2c <m2c.parquet> --cobrix <cobrix_parquet_dir_or_file>
    python scripts/compare_cobrix.py --test
"""

import argparse
import decimal
import sys
from decimal import Decimal
from typing import Any, Dict, List, Optional, Set, Tuple

try:
    import pyarrow as pa
    import pyarrow.parquet as pq
except ImportError:
    print(
        "Error: pyarrow is required. Install with: pip install pyarrow",
        file=sys.stderr,
    )
    sys.exit(1)


def normalize_column_name(name: str) -> str:
    """Normalize COBOL / Arrow column names.

    Strips group prefix (e.g. 'TRXN-REC.TRXN-ID' -> 'TRXN-ID')
    and replaces hyphens with underscores ('TRXN-ID' -> 'TRXN_ID').
    """
    leaf = name.rsplit(".", 1)[-1]
    return leaf.replace("-", "_").upper()


def normalize_value(val: Any) -> Any:
    """Normalize a scalar value to enable semantic equality without floats."""
    if val is None:
        return None
    # If Decimal with scale 0 (e.g. Decimal128(9, 0)), normalize to int for comparison with int32/int64
    if isinstance(val, Decimal):
        # If it has no fractional part, e.g. 324638715
        if val == val.to_integral():
            return int(val)
        return val
    if isinstance(val, (int,)):
        return int(val)
    if isinstance(val, (str,)):
        return str(val)
    if isinstance(val, bytes):
        return val
    return val


def compare_tables(
    m2c_table: pa.Table,
    cobrix_table: pa.Table,
    key_column: Optional[str] = "TRXN_ID",
    max_mismatches: int = 10,
) -> Tuple[bool, List[str]]:
    """Compare M2C and Cobrix tables semantically.

    Returns:
        (passed, messages_list)
    """
    lines: List[str] = []
    passed = True

    # 1. Map columns
    m2c_cols = {normalize_column_name(c): c for c in m2c_table.column_names}
    cobrix_cols = {normalize_column_name(c): c for c in cobrix_table.column_names}

    lines.append(f"M2C original columns:    {list(m2c_table.column_names)}")
    lines.append(f"Cobrix original columns: {list(cobrix_table.column_names)}")
    lines.append(f"Normalized columns:      {sorted(m2c_cols.keys())}")

    if set(m2c_cols.keys()) != set(cobrix_cols.keys()):
        missing_in_cobrix = set(m2c_cols.keys()) - set(cobrix_cols.keys())
        missing_in_m2c = set(cobrix_cols.keys()) - set(m2c_cols.keys())
        if missing_in_cobrix:
            lines.append(f"FAIL: Columns present in M2C but missing in Cobrix: {missing_in_cobrix}")
        if missing_in_m2c:
            lines.append(f"FAIL: Columns present in Cobrix but missing in M2C: {missing_in_m2c}")
        return False, lines

    norm_cols = sorted(m2c_cols.keys())

    # 2. Compare row counts
    m2c_rows = m2c_table.num_rows
    cobrix_rows = cobrix_table.num_rows
    lines.append(f"Row count: M2C = {m2c_rows}, Cobrix = {cobrix_rows}")

    if m2c_rows != cobrix_rows:
        lines.append(f"FAIL: Row count mismatch (M2C {m2c_rows} != Cobrix {cobrix_rows})")
        passed = False

    # Convert to Python dicts of lists
    m2c_dict = {col: m2c_table.column(orig).to_pylist() for col, orig in m2c_cols.items()}
    cobrix_dict = {col: cobrix_table.column(orig).to_pylist() for col, orig in cobrix_cols.items()}

    # 3. Check key column uniqueness and ordering
    has_key = key_column and key_column in norm_cols
    key_ordered = False

    if has_key:
        m2c_keys = [normalize_value(k) for k in m2c_dict[key_column]]
        cobrix_keys = [normalize_value(k) for k in cobrix_dict[key_column]]

        # Uniqueness check
        m2c_key_set: Set[Any] = set()
        m2c_dups = []
        for k in m2c_keys:
            if k in m2c_key_set:
                m2c_dups.append(k)
            m2c_key_set.add(k)

        cobrix_key_set: Set[Any] = set()
        cobrix_dups = []
        for k in cobrix_keys:
            if k in cobrix_key_set:
                cobrix_dups.append(k)
            cobrix_key_set.add(k)

        if m2c_dups:
            lines.append(f"FAIL: M2C has duplicate keys in {key_column}: {m2c_dups[:5]}")
            passed = False
        if cobrix_dups:
            lines.append(f"FAIL: Cobrix has duplicate keys in {key_column}: {cobrix_dups[:5]}")
            passed = False

        # Key set equality
        if m2c_key_set != cobrix_key_set:
            diff_m2c = m2c_key_set - cobrix_key_set
            diff_cobrix = cobrix_key_set - m2c_key_set
            if diff_m2c:
                lines.append(f"FAIL: Keys in M2C missing in Cobrix: {list(diff_m2c)[:5]}")
            if diff_cobrix:
                lines.append(f"FAIL: Keys in Cobrix missing in M2C: {list(diff_cobrix)[:5]}")
            passed = False

        if m2c_keys == cobrix_keys:
            lines.append(f"Key ordering: IDENTICAL across all {len(m2c_keys)} records.")
            key_ordered = True
        else:
            lines.append(f"Key ordering: DIFFERENT between writers; indexing records by {key_column}.")

    # 4. Record-by-record comparison
    mismatches: List[str] = []
    compared_records = 0

    if has_key and not key_ordered:
        # Build lookup table for cobrix by key
        cobrix_by_key = {}
        for row_idx in range(cobrix_rows):
            k = normalize_value(cobrix_dict[key_column][row_idx])
            cobrix_by_key[k] = row_idx

        for m2c_idx in range(m2c_rows):
            k = normalize_value(m2c_dict[key_column][m2c_idx])
            if k not in cobrix_by_key:
                continue
            cobrix_idx = cobrix_by_key[k]
            compared_records += 1
            for col in norm_cols:
                m2c_v = normalize_value(m2c_dict[col][m2c_idx])
                cobrix_v = normalize_value(cobrix_dict[col][cobrix_idx])
                if m2c_v != cobrix_v:
                    mismatches.append(
                        f"Record {key_column}={k}, Field '{col}': M2C={m2c_v!r} != Cobrix={cobrix_v!r}"
                    )
    else:
        min_rows = min(m2c_rows, cobrix_rows)
        for row_idx in range(min_rows):
            compared_records += 1
            row_id = (
                normalize_value(m2c_dict[key_column][row_idx])
                if has_key
                else f"row_{row_idx}"
            )
            for col in norm_cols:
                m2c_v = normalize_value(m2c_dict[col][row_idx])
                cobrix_v = normalize_value(cobrix_dict[col][row_idx])
                if m2c_v != cobrix_v:
                    mismatches.append(
                        f"Record #{row_idx} ({key_column}={row_id}), Field '{col}': M2C={m2c_v!r} != Cobrix={cobrix_v!r}"
                    )

    lines.append(f"Records compared: {compared_records}")
    if mismatches:
        passed = False
        lines.append(f"FAIL: Found {len(mismatches)} field value mismatch(es):")
        for m in mismatches[:max_mismatches]:
            lines.append(f"  - {m}")
        if len(mismatches) > max_mismatches:
            lines.append(f"  ... and {len(mismatches) - max_mismatches} more mismatches.")
    else:
        lines.append(f"Field comparison: ALL {len(norm_cols)} fields matched across all records.")

    return passed, lines


def run_self_test() -> bool:
    """Run internal test suite verifying comparator semantics."""
    print("Running compare_cobrix self-test...")
    schema_m2c = pa.schema([
        ("TRXN-REC.TRXN-ID", pa.decimal128(9, 0)),
        ("TRXN-REC.TRXN-DT", pa.string()),
        ("TRXN-REC.TRXN-TM", pa.string()),
        ("TRXN-REC.TRXN-AMNT", pa.decimal128(9, 2)),
    ])
    schema_cobrix = pa.schema([
        ("TRXN_ID", pa.int32()),
        ("TRXN_DT", pa.string()),
        ("TRXN_TM", pa.string()),
        ("TRXN_AMNT", pa.decimal128(9, 2)),
    ])

    data_m2c = [
        [Decimal("101"), Decimal("102")],
        ["20260901", "20260902"],
        ["120000", "130000"],
        [Decimal("150.50"), Decimal("-25.00")],
    ]
    data_cobrix = [
        [101, 102],
        ["20260901", "20260902"],
        ["120000", "130000"],
        [Decimal("150.50"), Decimal("-25.00")],
    ]

    t_m2c = pa.Table.from_arrays(data_m2c, schema=schema_m2c)
    t_cobrix = pa.Table.from_arrays(data_cobrix, schema=schema_cobrix)

    # 1. Test clean match
    ok, lines = compare_tables(t_m2c, t_cobrix)
    assert ok, f"Expected clean match, got failure: {lines}"

    # 2. Test value mismatch
    bad_data_cobrix = [
        [101, 102],
        ["20260901", "20260902"],
        ["120000", "130000"],
        [Decimal("150.50"), Decimal("-99.99")],  # mismatch
    ]
    t_bad = pa.Table.from_arrays(bad_data_cobrix, schema=schema_cobrix)
    ok2, lines2 = compare_tables(t_m2c, t_bad)
    assert not ok2, "Expected failure on value mismatch"

    # 3. Test duplicate key detection
    dup_data = [
        [101, 101],
        ["20260901", "20260902"],
        ["120000", "130000"],
        [Decimal("150.50"), Decimal("150.50")],
    ]
    t_dup = pa.Table.from_arrays(dup_data, schema=schema_cobrix)
    ok3, lines3 = compare_tables(t_m2c, t_dup)
    assert not ok3, "Expected failure on duplicate keys"

    # 4. Test reordered records
    reordered_cobrix = [
        [102, 101],
        ["20260902", "20260901"],
        ["130000", "120000"],
        [Decimal("-25.00"), Decimal("150.50")],
    ]
    t_reorder = pa.Table.from_arrays(reordered_cobrix, schema=schema_cobrix)
    ok4, lines4 = compare_tables(t_m2c, t_reorder)
    assert ok4, f"Expected reordered tables to match by key, got failure: {lines4}"

    print("Self-test: ALL 4 TESTS PASSED.")
    return True


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compare M2C Parquet with Spark/Cobrix Parquet"
    )
    parser.add_argument("--m2c", help="Path to M2C output Parquet file")
    parser.add_argument("--cobrix", help="Path to Spark/Cobrix Parquet file or directory")
    parser.add_argument(
        "--key-column", default="TRXN_ID", help="Key column name (default: TRXN_ID)"
    )
    parser.add_argument(
        "--max-mismatches",
        type=int,
        default=10,
        help="Max number of mismatches to display (default: 10)",
    )
    parser.add_argument(
        "--test", action="store_true", help="Run self-test on synthetic data"
    )

    args = parser.parse_args()

    if args.test:
        success = run_self_test()
        return 0 if success else 1

    if not args.m2c or not args.cobrix:
        parser.error("Both --m2c and --cobrix are required (or use --test)")

    try:
        m2c_table = pq.read_table(args.m2c)
    except Exception as e:
        print(f"Error reading M2C Parquet ({args.m2c}): {e}", file=sys.stderr)
        return 2

    try:
        cobrix_table = pq.read_table(args.cobrix)
    except Exception as e:
        print(f"Error reading Cobrix Parquet ({args.cobrix}): {e}", file=sys.stderr)
        return 2

    passed, report = compare_tables(
        m2c_table,
        cobrix_table,
        key_column=args.key_column,
        max_mismatches=args.max_mismatches,
    )

    print("=" * 60)
    print("M2C vs Spark/Cobrix Parquet Differential Report")
    print("=" * 60)
    for line in report:
        print(line)
    print("=" * 60)
    if passed:
        print("STATUS: PASS (100% semantically identical records)")
        return 0
    else:
        print("STATUS: FAIL (mismatches detected)")
        return 1


if __name__ == "__main__":
    sys.exit(main())
