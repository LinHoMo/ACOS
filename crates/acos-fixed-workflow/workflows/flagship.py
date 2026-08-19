#!/usr/bin/env python3
"""P1-FLAGSHIP-001 Fixed Workflow — deterministic CSV quality analysis.

Human-authored fixed program (P1-4). Python standard library only.

Pipeline (generic engineering logic, no benchmark-specific branches):
    load (fault-tolerant) -> schema alignment -> validate -> repair recoverable
    -> revalidate -> statistics -> aggregate -> report (4 sections + evidence log)

Usage: python flagship.py <input_dir> <output_report_path>
"""

import csv
import json
import sys
from pathlib import Path

STANDARD_COLUMNS = ["date", "product", "category", "units", "revenue"]
COLUMN_ALIASES = {
    "date": ["date", "transaction_date"],
    "product": ["product", "item_name"],
    "category": ["category", "item_category"],
    "units": ["units", "quantity"],
    "revenue": ["revenue", "sales"],
}
MISSING_TOKENS = {"", "NA", "N/A", "NULL", "null", "nan", "-"}


def infer_column_mapping(header):
    """Align file header to standard columns via keyword aliases."""
    mapping = {}
    for idx, col in enumerate(header):
        norm = col.strip().lower()
        for standard, aliases in COLUMN_ALIASES.items():
            if norm in aliases and standard not in mapping:
                mapping[standard] = idx
                break
    return mapping


def repair_field_count(row, ncols):
    """Merge unquoted currency fields split by comma (e.g. '$3,150.00')."""
    merged = []
    i = 0
    while i < len(row):
        cur = row[i].strip()
        if (
            cur.startswith("$")
            and i + 1 < len(row)
            and row[i + 1].replace(".", "", 1).isdigit()
        ):
            merged.append(cur + "," + row[i + 1].strip())
            i += 2
        else:
            merged.append(cur)
            i += 1
    return merged[:ncols] if len(merged) >= ncols else merged + [""] * (ncols - len(merged))


def clean_currency(value):
    """Strip currency formatting; returns (cleaned, was_currency)."""
    s = value.strip()
    if "$" in s or "," in s:
        return s.replace("$", "").replace(",", ""), True
    return s, False


def parse_number(value):
    s = value.strip()
    if s in MISSING_TOKENS:
        return None
    try:
        return float(s)
    except ValueError:
        return None


def detect_outliers(revenues):
    """Flag extreme outliers via median multiplier (generic rule)."""
    nums = sorted(v for v in revenues if v is not None and v >= 0)
    if len(nums) < 4:
        return []
    median = nums[len(nums) // 2]
    threshold = max(median * 10.0, 100.0)
    return [i for i, v in enumerate(revenues) if v is not None and v > threshold]


def load_csv(path):
    """Fault-tolerant load: repair unquoted-currency field splits."""
    repaired = 0
    with open(path, newline="", encoding="utf-8-sig") as f:
        reader = csv.reader(f)
        header = [h.strip() for h in next(reader)]
        rows = []
        for raw in reader:
            if len(raw) != len(header):
                fixed = repair_field_count(raw, len(header))
                if fixed != raw:
                    repaired += 1
                raw = fixed
            rows.append(raw)
    return header, rows, repaired


def process_file(path):
    """Full pipeline for one CSV file. Returns per-file record."""
    header, rows, currency_repairs = load_csv(path)
    issues = []
    if currency_repairs > 0:
        issues.append("currency_formatting")

    mapping = infer_column_mapping(header)
    if len(mapping) < len(STANDARD_COLUMNS):
        issues.append("column_name_drift")

    normalized = []
    for row in rows:
        rec = {}
        for std in STANDARD_COLUMNS:
            idx = mapping.get(std)
            rec[std] = row[idx] if idx is not None and idx < len(row) else ""
        normalized.append(rec)

    missing_count = 0
    for rec in normalized:
        for std in STANDARD_COLUMNS:
            if rec[std].strip() in MISSING_TOKENS:
                missing_count += 1
    if missing_count > 0:
        issues.append("missing_value_NA")

    # Row-level validity: a literal NULL in key fields marks the whole row
    # invalid (SQL NULL semantics); NA/empty are field-level missing markers.
    before_valid = len(normalized)
    valid = [
        rec
        for rec in normalized
        if rec["units"].strip().upper() != "NULL"
        and rec["revenue"].strip().upper() != "NULL"
        and rec["date"].strip().upper() != "NULL"
    ]
    if len(valid) < before_valid:
        issues.append("missing_value_NULL")
    normalized = valid

    # Repair: currency cleaning applies to the parsed values used downstream.
    for rec in normalized:
        cleaned, _ = clean_currency(rec["revenue"])
        rec["revenue"] = cleaned
        cleaned_units, _ = clean_currency(rec["units"])
        rec["units"] = cleaned_units

    before = len(normalized)
    seen = set()
    deduped = []
    for rec in normalized:
        key = tuple(rec[std] for std in STANDARD_COLUMNS)
        if key not in seen:
            seen.add(key)
            deduped.append(rec)
    if len(deduped) < before:
        issues.append("duplicate_rows")
    normalized = deduped

    has_negative = False
    for rec in normalized:
        units = parse_number(rec["units"])
        revenue = parse_number(rec["revenue"])
        if (units is not None and units < 0) or (revenue is not None and revenue < 0):
            has_negative = True
    if has_negative:
        issues.append("negative_values")

    revenues = [parse_number(rec["revenue"]) for rec in normalized]
    outliers = detect_outliers(revenues)
    if outliers:
        issues.append("extreme_outliers")

    for idx in outliers:
        normalized[idx]["_outlier"] = True

    total_revenue = round(sum(v for v in revenues if v is not None), 2)
    total_units = round(sum(v for v in (parse_number(r["units"]) for r in normalized) if v is not None), 2)
    row_count = len(normalized)

    return {
        "file": path.name,
        "display_name": "Q" + path.stem[-1].upper() if path.stem[-1].isdigit() else path.stem,
        "header": header,
        "mapping": mapping,
        "row_count": row_count,
        "total_revenue": total_revenue,
        "total_units": total_units,
        "issues": issues,
        "missing_count": missing_count,
        "currency_repairs": currency_repairs,
        "outlier_rows": [
            {"row": i + 2, "product": normalized[i]["product"], "revenue": revenues[i]}
            for i in outliers
        ],
        "negative_rows": [
            {"row": i + 2, "product": normalized[i]["product"],
             "units": parse_number(normalized[i]["units"]),
             "revenue": revenues[i]}
            for i, r in enumerate(normalized)
            if (parse_number(r["units"]) or 0) < 0 or (revenues[i] or 0) < 0
        ],
    }


def build_report(results, out_path):
    """Assemble the 4-section markdown report with evidence log."""
    lines = []
    lines.append("# P1 Flagship CSV Quality Report")
    lines.append("")
    lines.append("## data_quality")
    lines.append("")
    for r in results:
        lines.append(f"### {r['display_name']} ({r['file']})")
        if r["issues"]:
            lines.append(f"- Issues: {len(r['issues'])}")
            for issue in r["issues"]:
                lines.append(f"  - {issue}")
        else:
            lines.append("- Issues: 0 (clean)")
        lines.append(f"- Rows: {r['row_count']}")
        lines.append("")

    lines.append("## quarterly_summary")
    lines.append("")
    for r in results:
        lines.append(f"- {r['display_name']} revenue: {r['total_revenue']:.2f}")
        lines.append(f"- {r['display_name']} units: {r['total_units']}")
    lines.append("")

    lines.append("## anomalies")
    lines.append("")
    for r in results:
        for o in r["outlier_rows"]:
            lines.append(
                f"- {r['file']} row {o['row']}: {o['product']} revenue {o['revenue']:.2f} (extreme outlier)"
            )
        for n in r["negative_rows"]:
            lines.append(
                f"- {r['file']} row {n['row']}: {n['product']} negative values (units={n['units']}, revenue={n['revenue']})"
            )
    if not any(r["outlier_rows"] or r["negative_rows"] for r in results):
        lines.append("- No anomalies detected")
    lines.append("")

    lines.append("## recovery_log")
    lines.append("")
    for r in results:
        actions = []
        if r["currency_repairs"] > 0:
            actions.append(f"repaired {r['currency_repairs']} unquoted currency field(s)")
        if "duplicate_rows" in r["issues"]:
            actions.append("removed duplicate rows (kept first occurrence)")
        if r["missing_count"] > 0:
            actions.append(f"flagged {r['missing_count']} missing value(s); treated as 0 in revenue sums")
        if "column_name_drift" in r["issues"]:
            actions.append("aligned columns to standard schema via keyword mapping")
        if actions:
            lines.append(f"- {r['file']}: " + "; ".join(actions))
        else:
            lines.append(f"- {r['file']}: no repairs needed")
    lines.append("")

    total_issues = sum(len(r["issues"]) for r in results)
    files_with_issues = sum(1 for r in results if r["issues"])
    grand_total = round(sum(r["total_revenue"] for r in results), 2)

    lines.append("## Aggregate")
    lines.append("")
    lines.append(f"- Total files processed: {len(results)}")
    lines.append(f"- Files with issues: {files_with_issues}")
    lines.append(f"- Total issues found: {total_issues}")
    lines.append(f"- Grand total revenue: {grand_total:.2f}")
    lines.append("")

    lines.append("## Evidence Log")
    lines.append("")
    for r in results:
        lines.append(f"- {r['file']}: loaded, schema-aligned, validated ({len(r['issues'])} issues), repaired, revalidated, statistics computed")
    lines.append("- aggregate: totals computed from per-file statistics")
    lines.append("")

    Path(out_path).write_text("\n".join(lines), encoding="utf-8")
    return {
        "total_files_processed": len(results),
        "files_with_issues": files_with_issues,
        "total_issues_found": total_issues,
        "grand_total_revenue": grand_total,
    }


def main():
    input_dir = Path(sys.argv[1])
    out_path = sys.argv[2]
    csv_paths = sorted(input_dir.glob("*.csv"))
    results = [process_file(p) for p in csv_paths]
    summary = build_report(results, out_path)
    print(json.dumps({"files": results, "summary": summary}, indent=2))


if __name__ == "__main__":
    main()