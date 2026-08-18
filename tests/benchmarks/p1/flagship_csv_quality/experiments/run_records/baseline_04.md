<longcat_tool_call>write_file
<longcat_arg_key>content</longcat_arg_key>
<longcat_arg_value>#!/usr/bin/env python3
"""
Quarterly Sales CSV Data Quality Analyzer
Analyzes all quarterly sales CSV files in the target directory.
"""

import csv
import os
import re
from collections import defaultdict
from pathlib import Path

DATASET_DIR = Path("tests/benchmarks/p1/flagship_csv_quality/datasets")
OUTPUT_FILE = Path("tests/benchmarks/p1/flagship_csv_quality/datasets/quality_report.md")

# Standard column names (Q1 is the reference)
STD_COLUMNS = ["date", "product", "category", "units", "revenue"]

# Mapping from non-standard to standard column names
COLUMN_ALIASES = {
    "transaction_date": "date",
    "item_name": "product",
    "item_category": "category",
    "quantity": "units",
    "sales": "revenue",
}

# Missing value indicators
MISSING_INDICATORS = {"", "na", "n/a", "null", "none", "nan"}


def is_missing(val):
    """Check if a value represents missing data."""
    if val is None:
        return True
    return str(val).strip().lower() in MISSING_INDICATORS


def clean_numeric(val):
    """Clean a numeric value: remove $, commas, whitespace. Returns float or None."""
    if is_missing(val):
        return None
    s = str(val).strip().replace("$", "").replace(",", "").replace(" ", "")
    try:
        return float(s)
    except ValueError:
        return None


def clean_int(val):
    """Clean an integer value. Returns int or None."""
    if is_missing(val):
        return None
    s = str(val).strip().replace("$", "").replace(",", "").replace(" ", "")
    try:
        return int(float(s))
    except ValueError:
        return None


def detect_schema_issues(headers):
    """Detect schema inconsistencies and return mapping to standard names."""
    issues = []
    col_map = {}
    for h in headers:
        h_lower = h.strip().lower()
        if h_lower in COLUMN_ALIASES:
            col_map[h] = COLUMN_ALIASES[h_lower]
            issues.append(f"  - `{h}` → `{COLUMN_ALIASES[h_lower]}`")
        elif h_lower in STD_COLUMNS:
            col_map[h] = h_lower
        else:
            col_map[h] = h_lower
    return col_map, issues


def detect_formatting_issues(rows, col_map, revenue_col):
    """Detect formatting issues like currency symbols, commas."""
    issues = []
    for i, row in enumerate(rows):
        raw = row.get(revenue_col, "")
        if raw and ("$" in str(raw) or "," in str(raw)):
            issues.append(f"  - Row {i}: `{raw}`")
    return issues


def detect_missing(rows, col_map, units_col, revenue_col):
    """Detect missing values in units and revenue columns."""
    units_missing = []
    revenue_missing = []
    for i, row in enumerate(rows):
        if is_missing(row.get(units_col, "")):
            product = row.get(col_map.get("product", "product"), "?")
            category = row.get(col_map.get("category", "category"), "?")
            raw_val = row.get(units_col, "")
            units_missing.append(f"  - Row {i}: {product}, {category} (marked `{raw_val}`)")
        if is_missing(row.get(revenue_col, "")):
            product = row.get(col_map.get("product", "product"), "?")
            category = row.get(col_map.get("category", "category"), "?")
            revenue_missing.append(f"  - Row {i}: {product}, {category}")
    return units_missing, revenue_missing


def detect_duplicates(rows):
    """Detect duplicate rows."""
    seen = {}
    duplicates = []
    for i, row in enumerate(rows):
        key = tuple(row.values())
        if key in seen:
            duplicates.append(f"  - Row {i} duplicates Row {seen[key]}: `{', '.join(str(v) for v in key)}`")
        else:
            seen[key] = i
    return duplicates


def detect_negatives(rows, col_map, units_col, revenue_col):
    """Detect negative values in units and revenue."""
    neg_units = []
    neg_revenue = []
    for i, row in enumerate(rows):
        u = clean_int(row.get(units_col, ""))
        r = clean_numeric(row.get(revenue_col, ""))
        product = row.get(col_map.get("product", "product"), "?")
        category = row.get(col_map.get("category", "category"), "?")
        if u is not None and u < 0:
            neg_units.append(f"  - Row {i}: {product}, {category} — units = {u}")
        if r is not None and r < 0:
            neg_revenue.append(f"  - Row {i}: {product}, {category} — revenue = ${r:,.2f}")
    return neg_units, neg_revenue


def detect_outliers_iqr(values, label=""):
    """Detect outliers using IQR method."""
    clean = sorted([v for v in values if v is not None])
    if len(clean) < 4:
        return []
    n = len(clean)
    q1 = clean[n // 4]
    q3 = clean[3 * n // 4]
    iqr = q3 - q1
    lower = q1 - 1.5 * iqr
    upper = q3 + 1.5 * iqr
    outliers = []
    for i, v in enumerate(values):
        if v is not None and (v < lower or v > upper):
            outliers.append((i, v))
    return outliers


def detect_revenue_outliers(rows, col_map, units_col, revenue_col):
    """Detect revenue outliers where revenue/units ratio is extreme."""
    outliers = []
    # Compute unit prices for each product
    product_prices = defaultdict(list)
    for row in rows:
        u = clean_int(row.get(units_col, ""))
        r = clean_numeric(row.get(revenue_col, ""))
        product = row.get(col_map.get("product", "product"), "?")
        if u is not None and r is not None and u > 0 and r > 0:
            product_prices[product].append(r / u)

    # Average price per product
    avg_price = {}
    for p, prices in product_prices.items():
        avg_price[p] = sum(prices) / len(prices)

    for i, row in enumerate(rows):
        u = clean_int(row.get(units_col, ""))
        r = clean_numeric(row.get(revenue_col, ""))
        product = row.get(col_map.get("product", "product"), "?")
        if u is not None and r is not None and u > 0 and r > 0 and product in avg_price:
            expected = avg_price[product] * u
            if expected > 0 and (r / expected > 10 or r / expected < 0.1):
                outliers.append(
                    f"  - Row {i}: {product}, {row.get(col_map.get('category', 'category'), '?')} — "
                    f"revenue = ${r:,.2f} (expected ~${expected:,.0f} based on unit price)"
                )
    return outliers


def analyze_file(filepath):
    """Analyze a single CSV file and return results."""
    filename = os.path.basename(filepath)
    quarter = filename.replace("sales_", "").replace(".csv", "").upper()

    with open(filepath, "r", newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        headers = reader.fieldnames or []
        rows = list(reader)

    # Schema analysis
    col_map, schema_issues = detect_schema_issues(headers)

    # Resolve standard column names
    units_col = None
    revenue_col = None
    product_col = None
    category_col = None
    for orig, std in col_map.items():
        if std == "units":
            units_col = orig
        elif std == "revenue":
            revenue_col = orig
        elif std == "product":
            product_col = orig
        elif std == "category":
            category_col = orig

    # Formatting issues
    formatting_issues = detect_formattingIssues(rows, col_map, revenue_col) if revenue_col else []

    # Missing values
    units_missing, revenue_missing = detect_missing(rows, col_map, units_col, revenue_col)

    # Duplicates
    duplicates = detect_duplicates(rows)

    # Negatives
    neg_units, neg_revenue = detect_negatives(rows, col_map, units_col, revenue_col)

    # Revenue outliers
    revenue_outliers = detect_revenue_outliers(rows, col_map, units_col, revenue_col)

    # Units outliers (IQR)
    units_values = [clean_int(row.get(units_col, "")) for row in rows]
    units_outliers_raw = detect_outliers_iqr(units_values)
    units_outliers = []
    for idx, val in units_outliers_raw:
        product = rows[idx].get(product_col, "?")
        category = rows[idx].get(category_col, "?")
        units_outliers.append(f"  - Row {idx}: {product}, {category} — units = {val}")

    # Compute statistics
    total_rows = len(rows)
    valid_units = [clean_int(row.get(units_col, "")) for row in rows]
    valid_revenue = [clean_numeric(row.get(revenue_col, "")) for row in rows]
    valid_units_clean = [v for v in valid_units if v is not None]
    valid_revenue_clean = [v for v in valid_revenue if v is not None]

    total_units = sum(valid_units_clean) if valid_units_clean else 0
    total_revenue = sum(valid_revenue_clean) if valid_revenue_clean else 0
    avg_units = total_units / len(valid_units_clean) if valid_units_clean else 0
    avg_revenue = total_revenue / len(valid_revenue_clean) if valid_revenue_clean else 0
    units_range = (min(valid_units_clean), max(valid_units_clean)) if valid_units_clean else (0, 0)
    revenue_range = (min(valid_revenue_clean), max(valid_revenue_clean)) if valid_revenue_clean else (0, 0)

    # Category breakdown
    cat_stats = defaultdict(lambda: {"rows": 0, "revenue": 0.0, "units": 0})
    for row in rows:
        cat = row.get(category_col, "Unknown")
        u = clean_int(row.get(units_col, ""))
        r = clean_numeric(row.get(revenue_col, ""))
        cat_stats[cat]["rows"] += 1
        if r is not None:
            cat_stats[cat]["revenue"] += r
        if u is not None:
            cat_stats[cat]["units"] += u

    return {
        "filename": filename,
        "quarter": quarter,
        "total_rows": total_rows,
        "valid_revenue_rows": len(valid_revenue_clean),
        "valid_units_rows": len(valid_units_clean),
        "total_revenue": total_revenue,
        "total_units": total_units,
        "avg_revenue": avg_revenue,
        "avg_units": avg_units,
        "revenue_range": revenue_range,
        "units_range": units_range,
        "category_stats": dict(cat_stats),
        "schema_issues": schema_issues,
        "formatting_issues": formatting_issues,
        "units_missing": units_missing,
        "revenue_missing": revenue_missing,
        "duplicates": duplicates,
        "neg_units": neg_units,
        "neg_revenue": neg_revenue,
        "revenue_outliers": revenue_outliers,
        "units_outliers": units_outliers,
    }


def generate_report(results):
    """Generate the consolidated quality report."""
    lines = []
    lines.append("# Quarterly Sales Data Quality Report")
    lines.append("")

    # === Quarterly Summary ===
    lines.append("## Quarterly Summary")
    lines.append("")
    lines.append("| Quarter | Total Rows | Total Revenue | Total Units | Avg Revenue | Avg Units |")
    lines.append("|---------|-----------|---------------|-------------|-------------|-----------|")
    for r in results:
        lines.append(
            f"| {r['quarter']} | {r['total_rows']} | ${r['total_revenue']:,.2f} | {r['total_units']} | "
            f"${r['avg_revenue']:,.2f} | {r['avg_units']:.1f} |"
        )
    lines.append("")

    # Category Breakdown
    lines.append("### Category Breakdown by Quarter")
    lines.append("")
    for r in results:
        lines.append(f"#### {r['quarter']}")
        lines.append("")
        lines.append("| Category | Rows | Revenue | Units |")
        lines.append("|----------|------|---------|-------|")
        for cat, stats in sorted(r["category_stats"].items()):
            lines.append(
                f"| {cat} | {stats['rows']} | ${stats['revenue']:,.2f} | {stats['units']} |"
            )
        lines.append("")

    # === Anomalies ===
    lines.append("## Anomalies and Data Quality Issues")
    lines.append("")

    # Schema Inconsistencies
    schema_found = False
    for r in results:
        if r["schema_issues"]:
            if not schema_found:
                lines.append("### Schema Inconsistencies")
                lines.append("")
                schema_found = True
            lines.append(f"- **{r['quarter']}** ({r['filename']}): Column names differ from other quarters:")
            lines.extend(r["schema_issues"])
            lines.append("")

    # Formatting Issues
    fmt_found = False
    for r in results:
        if r["formatting_issues"]:
            if not fmt_found:
                lines.append("### Formatting Issues")
                lines.append("")
                fmt_found = True
            lines.append(
                f"- **{r['quarter']}** ({r['filename']}): Revenue values contain currency symbols (`$`) and "
                f"comma-separated thousands (e.g., `{r['formatting_issues'][0].strip().split(': ')[1] if r['formatting_issues'] else 'N/A'}`). "
                f"These must be cleaned before numeric analysis."
            )
            lines.append("")

    # Missing Values
    missing_found = False
    for r in results:
        if r["units_missing"] or r["revenue_missing"]:
            if not missing_found:
                lines.append("### Missing Values")
                lines.append("")
                missing_found = True
            if r["units_missing"]:
                lines.append(
                    f"- **{r['quarter']}** ({r['filename']}): {len(r['units_missing'])} missing value(s) in "
                    f"`units` column:"
                )
                lines.extend(r["units_missing"])
                lines.append("")
            if r["revenue_missing"]:
                lines.append(
                    f"- **{r['quarter']}** ({r['filename']}): {len(r['revenue_missing'])} missing value(s) in "
                    f"`revenue` column:"
                )
                lines.extend(r["revenue_missing"])
                lines.append("")

    # Duplicate Rows
    dup_found = False
    for r in results:
        if r["duplicates"]:
            if not dup_found:
                lines.append("### Duplicate Rows")
                lines.append("")
                dup_found = True
            lines.append(f"- **{r['quarter']}** ({r['filename']}): {len(r['duplicates'])} duplicate row(s) detected:")
            lines.extend(r["duplicates"])
            lines.append("")

    # Negative Values
    neg_found = False
    for r in results:
        if r["neg_units"] or r["neg_revenue"]:
            if not neg_found:
                lines.append("### Negative Values")
                lines.append("")
                neg_found = True
            if r["neg_units"]:
                lines.append(f"- **{r['quarter']}** ({r['filename']}): {len(r['neg_units'])} row(s) with negative units:")
                lines.extend(r["neg_units"])
                lines.append("")
            if r["neg_revenue"]:
                lines.append(
                    f"- **{r['quarter']}** ({r['filename']}): {len(r['neg_revenue'])} row(s) with negative revenue:"
                )
                lines.extend(r["neg_revenue"])
                lines.append("")

    # Revenue Outliers
    out_found = False
    for r in results:
        if r["revenue_outliers"]:
            if not out_found:
                lines.append("### Revenue Outliers (Extreme Values)")
                lines.append("")
                out_found = True
            lines.append(
                f"- **{r['quarter']}** ({r['filename']}): {len(r['revenue_outliers'])} extreme revenue outlier(s) "
                f"detected (likely data entry errors — values 100x normal):"
            )
            lines.extend(r["revenue_outliers"])
            lines.append("")

    # Units Outliers
    uout_found = False
    for r in results:
        if r["units_outliers"]:
            if not uout_found:
                lines.append("### Units Outliers (IQR Method)")
                lines.append("")
                uout_found = True
            lines.append(
                f"- **{r['quarter']}** ({r['filename']}): {len(r['units_outliers'])} potential units outlier(s):"
            )
            lines.extend(r["units_outliers"])
            lines.append("")

    # === Detailed Statistics ===
    lines.append("## Detailed Statistics")
    lines.append("")
    for r in results:
        lines.append(f"### {r['quarter']}")
        lines.append("")
        lines.append(f"- Total rows: {r['total_rows']}")
        lines.append(f"- Valid revenue rows: {r['valid_revenue_rows']}")
        lines.append(f"- Valid units rows: {r['valid_units_rows']}")
        lines.append(f"- Total revenue: ${r['total_revenue']:,.2f}")
        lines.append(f"- Total units: {r['total_units']}")
        lines.append(f"- Average revenue per transaction: ${r['avg_revenue']:,.2f}")
        lines.append(f"- Average units per transaction: {r['avg_units']:.1f}")
        lines.append(
            f"- Revenue range: ${r['revenue_range'][0]:,.2f} - ${r['revenue_range'][1]:,.2f}"
        )
        lines.append(f"- Units range: {r['units_range'][0]} - {r['units_range'][1]}")
