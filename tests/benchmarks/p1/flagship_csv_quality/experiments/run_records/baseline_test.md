<longcat_tool_call>write_file
<longcat_arg_key>content</longcat_arg_key>
<longcat_arg_value>import csv
import os
import sys
from pathlib import Path
from collections import defaultdict
import json
from datetime import datetime

DATA_DIR = Path("tests/benchmarks/p1/flagship_csv_quality/datasets/")
REPORT_PATH = Path("consolidated_sales_report.md")
ANOMALY_CSV_PATH = Path("sales_anomalies.csv")

# Column name mappings to unify schemas
COLUMN_ALIASES = {
    "date": ["date", "transaction_date"],
    "product": ["product", "item_name"],
    "category": ["category", "item_category"],
    "units": ["units", "quantity"],
    "revenue": ["revenue", "sales"],
}

MISSING_TOKENS = {"", "na", "n/a", "null", "none", "nan"}


def is_missing(val):
    return val is None or str(val).strip().lower() in MISSING_TOKENS


def parse_number(val):
    """Try to parse a number, handling currency symbols and commas."""
    if val is None:
        return None
    s = str(val).strip()
    if is_missing(s):
        return None
    # Remove currency symbols
    s = s.replace("$", "").replace("€", "").replace("£", "")
    # Remove thousands separators (commas)
    s = s.replace(",", "")
    try:
        return float(s)
    except ValueError:
        return None


def detect_schema(header_row):
    """Map a file's columns to the canonical schema."""
    mapping = {}
    for canonical, aliases in COLUMN_ALIASES.items():
        for col in header_row:
            if col.strip().lower() in [a.lower() for a in aliases]:
                mapping[canonical] = col
                break
    return mapping


def find_files():
    return sorted([f for f in DATA_DIR.glob("sales_q*.csv") if f.is_file()])


def normalize_row(row, schema, header):
    """Normalize a row to canonical column names, handling extra/missing columns."""
    result = {}
    for canonical, original in schema.items():
        if original in header:
            idx = header.index(original)
            if idx < len(row):
                result[canonical] = row[idx].strip() if row[idx] else ""
            else:
                result[canonical] = ""
        else:
            result[canonical] = ""
    return result


def detect_corrupted_split(row, expected_cols):
    """Detect if a row has more columns than expected due to commas in quoted currency values.
    When CSV parser splits '$3,150.00' incorrectly, it creates extra columns.
    Returns the repaired row if corruption is detected, else None."""
    if len(row) <= expected_cols:
        return None
    
    # Look for pattern: column starts with "$" and next column continues the number
    # e.g., ['$3', '150.00'] -> '$3,150.00'
    repaired = list(row)
    while len(repaired) > expected_cols:
        merged = False
        for i in range(len(repaired) - 1):
            s1 = repaired[i].strip() if repaired[i] else ""
            s2 = repaired[i + 1].strip() if repaired[i + 1] else ""
            # Pattern: "$X" followed by "Y" where together they form a number
            if s1.startswith("$") and s2.replace(".", "").replace(",", "").isdigit():
                repaired[i] = s1 + "," + s2
                repaired.pop(i + 1)
                merged = True
                break
            # Pattern: "$X" followed by "Y" (general case)
            elif s1.startswith("$"):
                repaired[i] = s1 + s2
                repaired.pop(i + 1)
                merged = True
                break
        if not merged:
            break
    
    if len(repaired) == expected_cols:
        return repaired
    return None


def analyze_file(filepath):
    """Analyze a single CSV file and return statistics and issues."""
    filename = filepath.name
    quarter = filename.replace("sales_", "").replace(".csv", "").upper()
    
    stats = {
        "file": filename,
        "quarter": quarter,
        "total_rows_raw": 0,
        "total_rows_clean": 0,
        "total_revenue": 0.0,
        "total_units": 0.0,
        "schema_issues": [],
        "issues": [],
        "category_revenue": defaultdict(float),
        "category_units": defaultdict(float),
        "product_revenue": defaultdict(float),
        "product_units": defaultdict(float),
        "anomalies": [],
        "repaired_rows": [],
    }
    
    with open(filepath, "r", newline="", encoding="utf-8") as f:
        raw_content = f.read()
    
    lines = raw_content.strip().split("\n")
    
    # Parse header
    reader = csv.reader(lines)
    header = next(reader)
    expected_cols = len(header)
    
    schema = detect_schema(header)
    
    # Check for schema differences
    canonical_expected = set(COLUMN_ALIASES.keys())
    canonical_found = set(schema.keys())
    missing_cols = canonical_expected - canonical_found
    extra_cols = set(h.strip().lower() for h in header) - set(
        alias for aliases in COLUMN_ALIASES.values() for alias in aliases
    )
    
    if missing_cols:
        stats["schema_issues"].append(f"Missing canonical columns: {missing_cols}")
    if extra_cols:
        stats["schema_issues"].append(f"Unrecognized columns: {extra_cols}")
    
    seen_rows = set()
    row_num = 1  # start at 1 (header is line 1)
    
    for line in lines[1:]:
        row_num += 1
        row = next(csv.reader([line]))
        
        stats["total_rows_raw"] += 1
        
        # Detect and repair corrupted split
        if len(row) > expected_cols:
            repaired = detect_corrupted_split(row, expected_cols)
            if repaired:
                stats["repaired_rows"].append({
                    "line": row_num,
                    "original": row,
                    "repaired": repaired,
                })
                row = repaired
            else:
                stats["issues"].append({
                    "line": row_num,
                    "type": "malformed_row",
                    "detail": f"Expected {expected_cols} columns, got {len(row)}",
                })
                continue
        
        if len(row) < expected_cols:
            stats["issues"].append({
                "line": row_num,
                "type": "malformed_row",
                "detail": f"Expected {expected_cols} columns, got {len(row)}",
            })
            continue
        
        norm = normalize_row(row, schema, header)
        
        date_val = norm.get("date", "")
        product_val = norm.get("product", "")
        category_val = norm.get("category", "")
        units_raw = norm.get("units", "")
        revenue_raw = norm.get("revenue", "")
        
        # Check for missing values
        missing_fields = []
        if is_missing(units_raw):
            missing_fields.append("units")
        if is_missing(revenue_raw):
            missing_fields.append("revenue")
        
        if missing_fields:
            for field in missing_fields:
                stats["issues"].append({
                    "line": row_num,
                    "type": "missing_value",
                    "field": field,
                    "value": units_raw if field == "units" else revenue_raw,
                })
        
        units_val = parse_number(units_raw)
        revenue_val = parse_number(revenue_raw)
        
        # Check for negative values
        if units_val is not None and units_val < 0:
            stats["issues"].append({
                "line": row_num,
                "type": "negative_value",
                "field": "units",
                "value": units_val,
            })
        if revenue_val is not None and revenue_val < 0:
            stats["issues"].append({
                "line": row_num,
                "type": "negative_value",
                "field": "revenue",
                "value": revenue_val,
            })
        
        # Dedup check
        row_key = tuple(v for v in row)
        if row_key in seen_rows:
            stats["issues"].append({
                "line": row_num,
                "type": "duplicate",
                "detail": f"Duplicate row detected",
            })
        else:
            seen_rows.add(row_key)
        
        # Accumulate stats
        if units_val is not None:
            stats["total_units"] += units_val
        if revenue_val is not None:
            stats["total_revenue"] += revenue_val
        
        cat = category_val if category_val else "Unknown"
        prod = product_val if product_val else "Unknown"
        
        if units_val is not None:
            stats["category_units"][cat] += units_val
            stats["product_units"][prod] += units_val
        if revenue_val is not None:
            stats["category_revenue"][cat] += revenue_val
            stats["product_revenue"][prod] += revenue_val
        
        stats["total_rows_clean"] += 1
    
    # Detect outliers in this file (using IQR method on per-product average price)
    product_avg_prices = defaultdict(list)
    for line in lines[1:]:
        row = next(csv.reader([line]))
        if len(row) > expected_cols:
            repaired = detect_corrupted_split(row, expected_cols)
            if repaired:
                row = repaired
        if len(row) != expected_cols:
            continue
        
        norm = normalize_row(row, schema, header)
        u = parse_number(norm.get("units", ""))
        r = parse_number(norm.get(revenue_raw, ""))
        
        # Re-parse with correct canonical name
        rev_raw = norm.get("revenue", "")
        units_raw = norm.get("units", "")
        u = parse_number(units_raw)
        r = parse_number(rev_raw)
        
        prod = norm.get("product", "") or "Unknown"
        if u and r and u > 0 and r > 0:
            product_avg_prices[prod].append(r / u)
    
    for prod, prices in product_avg_prices.items():
        if len(prices) >= 3:
            prices_sorted = sorted(prices)
            q1 = prices_sorted[len(prices_sorted) // 4]
            q3 = prices_sorted[3 * len(prices_sorted) // 4]
            iqr = q3 - q1
            lower = q1 - 1.5 * iqr
            upper = q3 + 1.5 * iqr
            for i, p in enumerate(prices):
                if p < lower or p > upper:
                    stats["anomalies"].append({
                        "quarter": quarter,
                        "product": prod,
                        "avg_price": p,
                        "expected_range": (lower, upper),
                        "type": "outlier_pricing",
                    })
    
    return stats


def detect_global_outliers(all_stats):
    """Detect outliers across all quarters for each product."""
    product_quarterly_revenue = defaultdict(list)
    product_quarterly_units = defaultdict(list)
    
    for s in all_stats:
        for prod, rev in s["product_revenue"].items():
            product_quarterly_revenue[prod].append({
                "quarter": s["quarter"],
                "revenue": rev,
            })
        for prod, units in s["product_units"].items():
            product_quarterly_units[prod].append({
                "quarter": s["quarter"],
                "units": units,
            })
    
    global_anomalies = []
    
    # Detect revenue outliers per product
    for prod, quarters in product_quarterly_revenue.items():
        if len(quarters) >= 3:
            revenues = sorted([q["revenue"] for q in quarters])
            q1_idx = len(revenues) // 4
            q3_idx = 3 * len(revenues) // 4
            q1_val = revenues[q1_idx]
            q3_val = revenues[q3_idx]
            iqr = q3_val - q1_val
            lower = q1_val - 1.5 * iqr
            upper = q3_val + 1.5 * iqr
            for q in quarters:
                if q["revenue"] < lower or q["revenue"] > upper:
                    global_anomalies.append({
                        "product": prod,
                        "quarter": q["quarter"],
                        "metric": "revenue",
                        "value": q["revenue"],
                        "expected_min": lower,
                        "expected_max": upper,
                        "type": "global_revenue_outlier",
                    })
    
    # Detect unit outliers per product
    for prod, quarters in product_quarterly_units.items():
        if len(quarters) >= 3:
            units_list = sorted([q["units"] for q in quarters])
            q1_idx = len(units_list) // 4
            q3_idx = 3 * len(units_list) // 4
            q1_val = units_list[q1_idx]
            q3_val = units_list[q3_idx]
            iqr = q3_val - q1_val
            lower = q1_val - 1.5 * iqr
            upper = q3_val + 1.5 * iqr
            for q in quarters:
                if q["units"] < lower or q["units"] > upper:
                    global_anomalies.append({
                        "product": prod,
                        "quarter": q["quarter"],
                        "metric": "units",
                        "value": q["units"],
                        "expected_min": lower,
                        "expected_max": upper,
                        "type": "global_units_outlier",
                    })
    
    return global_anomalies


def generate_report(all_stats, global_anomalies, output_path):
    """Generate the consolidated report."""
    total_revenue = sum(s["total_revenue"] for s in all_stats)
    total_units = sum(s["total_units"] for s in all_stats)
    total_issues = sum(len(s["issues"]) for s in all_stats)
    total_rows = sum(s["total_rows_raw"] for s in all_stats)
    total_repaired = sum(len(s["repaired_rows"]) for s in all_stats)
    
    lines = []
    lines.append("# Consolidated Sales Quality Report")
    lines.append("")
    lines.append(f"Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    lines.append("")
    
    # Executive summary
    lines.append("## Executive Summary")
    lines.append("")
    lines.append(f"| Metric | Value |")
    lines.append(f"|--------|-------|")
    lines.append(f"| Total Files Analyzed | {len(all_stats)} |")
    lines.append(f"| Total Data Rows | {total_rows} |")
    lines.append(f"| Total Revenue | ${total_revenue:,.2f} |")
    lines.append(f"| Total Units Sold | {total_units:,.0f} |")
    lines.append(f"| Total Data Quality Issues | {total_issues} |")
    lines.append(f"| Total Repaired Rows (format) | {total_repaired} |")
    lines.append("")
    
    # Quarterly Summary
    lines.append("## Quarterly Summary")
    lines.append("")
    
    # File-level stats table
    lines.append("### File Statistics")
    lines.append("")
    lines.append("| File | Quarter | Raw Rows | Clean Rows | Revenue | Units | Issues | Schema Issues |")
    lines.append("|------|---------|----------|------------|---------|-------|--------|---------------|")
    
    for s in all_stats:
        schema_issues = "; ".join(s["schema_issues"]) if s["schema_issues"] else "None"
        lines.append(
            f"| {s['file']} | {s['quarter']} | {s['total_rows_raw']} | "
            f"{s['total_rows_clean']} | ${s['total_revenue']:,.2f} | {s['total_units']:,.0f} | "
            f"{len(s['issues'])} | {schema_issues} |"
        )
    lines.append("")
    
    # Revenue by category per quarter
    lines.append("### Revenue by Category per Quarter")
    lines.append("")
    all_categories = set()
    for s in all_stats:
        all_categories.update(s["category_revenue"].keys())
    all_categories = sorted(all_categories)
    
    header_cols = ["Category"] + [s["quarter"] for s in all_stats]
    lines.append("| " + " | ".join(header_cols) + " |")
    lines.append("| " + " | ".join(["---"] * len(header_cols)) + " |")
    
    for cat in all_categories:
        values = []
        for s in all_stats:
            rev = s["category_revenue"].get(cat, 0)
            values.append(f"${rev:,.2f}")
        lines.append("| " + cat + " | " + " | ".join(values) + " |")
    lines.append("")
    
    # Revenue by product per quarter
    lines.append("### Revenue by Product per Quarter")
    lines.append("")
    all_products = set()
    for s in all_stats:
        all_products.update(s["product_revenue"].keys())
    all_products = sorted(all_products)
    
    header_cols = ["Product"] + [s["quarter"] for s in all_stats]
    lines.append("| " + " | ".join(header_cols) + " |")
    lines.append("| " + " | ".join(["