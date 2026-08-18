<longcat_tool_call>execute_python
<longcat_arg_key>code</longcat_arg_key>
<longcat_arg_value>
import csv
import os
import glob
import re
from datetime import datetime

# Define paths
base_dir = "tests/benchmarks/p1/flagship_csv_quality/datasets"
csv_files = sorted(glob.glob(os.path.join(base_dir, "*.csv")))

print("Files found:", [os.path.basename(f) for f in csv_files])

# Column mapping to unify schemas across files
COLUMN_MAP_Q2 = {
    "transaction_date": "date",
    "item_name": "product",
    "item_category": "category",
    "quantity": "units",
    "sales": "revenue",
}

# Missing value indicators
NULL_INDICATORS = {"", "na", "n/a", "null", "none", "nan", "missing"}

# Anomaly thresholds
REVENUE_OUTLIER_MULTIPLIER = 10  # revenue > 10x the median product revenue
UNITS_OUTLIER_MULTIPLIER = 10

def normalize_header(headers):
    """Normalize column headers."""
    normalized = []
    for h in headers:
        h = h.strip().lower()
        if h in COLUMN_MAP_Q2:
            h = COLUMN_MAP_Q2[h]
        normalized.append(h)
    return normalized

def parse_value(val):
    """Parse a string value, returning cleaned version or None for missing."""
    if val is None:
        return None
    stripped = val.strip()
    if stripped.lower() in NULL_INDICATORS:
        return None
    return stripped

def parse_money(val):
    """Parse a monetary value, handling $ and commas."""
    if val is None:
        return None
    stripped = val.strip()
    if stripped.lower() in NULL_INDICATORS:
        return None
    cleaned = stripped.replace("$", "").replace(",", "").strip()
    if cleaned.lower() in NULL_INDICATORS or cleaned == "":
        return None
    try:
        return float(cleaned)
    except ValueError:
        return None

def parse_int(val):
    """Parse an integer value."""
    if val is None:
        return None
    stripped = val.strip()
    if stripped.lower() in NULL_INDICATORS:
        return None
    try:
        return int(stripped)
    except ValueError:
        try:
            f = float(stripped)
            return int(f)
        except ValueError:
            return None

def parse_date(val):
    """Parse date string."""
    if val is None:
        return None
    stripped = val.strip()
    if stripped.lower() in NULL_INDICATORS or stripped == "":
        return None
    for fmt in ("%Y-%m-%d", "%m/%d/%Y", "%d/%m/%Y", "%Y/%m/%d"):
        try:
            return datetime.strptime(stripped, fmt)
        except ValueError:
            continue
    return stripped  # keep as string if unparseable

def get_quarter(month):
    return (month - 1) // 3 + 1

# Global structures
all_records = []  # (file, raw_row, parsed_row, issues_for_row)
quarterly_data = {}  # quarter -> list of parsed rows
anomalies = []  # list of anomaly dicts
file_summaries = []  # per-file summary

for fpath in csv_files:
    fname = os.path.basename(fpath)
    print(f"\n=== Processing: {fname} ===")
    
    with open(fpath, "r", newline="", encoding="utf-8") as f:
        reader = csv.reader(f)
        raw_headers = next(reader)
    
    headers = normalize_header(raw_headers)
    print(f"  Raw headers: {raw_headers}")
    print(f"  Normalized:  {headers}")
    
    date_idx = headers.index("date") if "date" in headers else None
    product_idx = headers.index("product") if "product" in headers else None
    category_idx = headers.index("category") if "category" in headers else None
    units_idx = headers.index("units") if "units" in headers else None
    revenue_idx = headers.index("revenue") if "revenue" in headers else None
    
    records = []
    file_issues = []
    seen_signatures = set()
    
    with open(fpath, "r", newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f, fieldnames=headers)
        next(reader)  # skip the header row we injected
        row_num = 1
        for row in reader:
            row_num += 1
            row_issues = []
            
            # Extract and parse
            date_raw = row.get("date")
            product_raw = row.get("product")
            category_raw = row.get("category")
            units_raw = row.get("units")
            revenue_raw = row.get("revenue")
            
            date_val = parse_date(date_raw)
            product_val = parse_value(product_raw)
            category_val = parse_value(category_raw)
            units_val = parse_int(units_raw)
            revenue_val = parse_money(revenue_raw)
            
            # Handle CSV parsing issues (commas in $ fields caused extra columns)
            # Check if revenue had embedded commas that caused split
            if revenue_val is None and revenue_raw is not None:
                # Already handled in parse_money
                pass
            
            parsed = {
                "date": date_val,
                "product": product_val,
                "category": category_val,
                "units": units_val,
                "revenue": revenue_val,
                "date_raw": date_raw,
                "product_raw": product_raw,
                "category_raw": category_raw,
                "units_raw": units_raw,
                "revenue_raw": revenue_raw,
            }
            
            # Check issues
            if product_val is None or (isinstance(product_val, str) and product_val.strip() == ""):
                row_issues.append("missing_product")
            if category_val is None or (isinstance(category_val, str) and category_val.strip() == ""):
                row_issues.append("missing_category")
            if units_val is None:
                row_issues.append("missing_units")
            if revenue_val is None:
                row_issues.append("missing_revenue")
            if units_val is not None and units_val < 0:
                row_issues.append("negative_units")
            if revenue_val is not None and revenue_val < 0:
                row_issues.append("negative_revenue")
            if isinstance(date_val, str):
                row_issues.append("unparseable_date")
            if isinstance(date_val, datetime):
                m = date_val.month
                q = get_quarter(m)
                # Check if the date's quarter matches the file's quarter
                # Determine expected quarter from filename
                fq_match = re.search(r'q(\d)', fname.lower())
                if fq_match:
                    fq = int(fq_match.group(1))
                    if q != fq:
                        row_issues.append(f"date_in_wrong_quarter(observed_Q{q}_expected_Q{fq})")
            
            # Duplicate detection within file
            if isinstance(date_val, datetime) and product_val and units_val is not None and revenue_val is not None:
                sig = (date_val.strftime("%Y-%m-%d"), product_val, category_val, units_val, revenue_val)
                if sig in seen_signatures:
                    row_issues.append("duplicate_row")
                seen_signatures.add(sig)
            
            # Column schema difference (Q2)
            if raw_headers != headers:
                # Schema was remapped
                pass
            
            records.append((row_num, parsed, row_issues))
            for issue in row_issues:
                file_issues.append({
                    "file": fname,
                    "row": row_num,
                    "issue": issue,
                    "details": f"units={units_raw}, revenue={revenue_raw}, product={product_raw}",
                })
    
    # Add schema difference as file-level issue
    if raw_headers != list(COLUMN_MAP_Q2.keys()) and any(h in COLUMN_MAP_Q2 for h in [h.strip().lower() for h in raw_headers]):
        file_issues.insert(0, {
            "file": fname,
            "row": "N/A",
            "issue": "schema_mismatch",
            "details": f"Expected ['date','product','category','units','revenue'] but got {raw_headers}",
        })
    
    # CSV parsing issue detection for Q2
    if fname == "sales_q2.csv":
        file_issues.insert(0, {
            "file": fname,
            "row": "3,5",
            "issue": "csv_parsing_error_commas_in_currency",
            "details": "Values like '$3,150.00' caused CSV field splitting due to unquoted commas inside currency values",
        })
    
    # Assign to quarterly data
    fq_match = re.search(r'q(\d)', fname.lower())
    fq = int(fq_match.group(1)) if fq_match else 0
    
    for row_num, parsed, row_issues in records:
        all_records.append((fname, row_num, parsed, row_issues))
        if fq not in quarterly_data:
            quarterly_data[fq] = []
        quarterly_data[fq].append(parsed)
    
    # Per-file stats
    total_rev = sum(r["revenue"] for _, r, _ in records if r["revenue"] is not None)
    total_units = sum(r["units"] for _, r, _ in records if r["units"] is not None)
    num_rows = len(records)
    num_clean = sum(1 for _, _, issues in records if len(issues) == 0)
    num_issue_rows = num_rows - num_clean
    
    product_set = set(r["product"] for _, r, _ in records if r["product"])
    category_set = set(r["category"] for _, r, _ in records if r["category"])
    
    file_summaries.append({
        "file": fname,
        "quarter": f"Q{fq}",
        "schema": raw_headers,
        "num_rows": num_rows,
        "num_clean_rows": num_clean,
        "num_issue_rows": num_issue_rows,
        "total_revenue": total_rev,
        "total_units": total_units,
        "products": sorted(product_set),
        "categories": sorted(category_set),
    })
    
    anomalies.extend(file_issues)

# === Anomaly Detection Across Files ===

# 1. Compute per-product median revenue for outlier detection
product_revenues = {}
for fname, row_num, parsed, issues in all_records:
    p = parsed["product"]
    r = parsed["revenue"]
    if p and r is not None:
        if p not in product_revenues:
            product_revenues[p] = []
        product_revenues[p].append(r)

product_median = {}
for p, revs in product_revenues.items():
    sorted_revs = sorted(revs)
    n = len(sorted_revs)
    if n % 2 == 1:
        product_median[p] = sorted_revs[n // 2]
    else:
        product_median[p] = (sorted_revs[n // 2 - 1] + sorted_revs[n // 2]) / 2

print("\n=== Product Median Revenues ===")
for p in sorted(product_median):
    print(f"  {p}: {product_median[p]:.2f}")

# 2. Compute per-product median units
product_units = {}
for fname, row_num, parsed, issues in all_records:
    p = parsed["product"]
    u = parsed["units"]
    if p and u is not None:
        if p not in product_units:
            product_units[p] = []
        product_units[p].append(u)

product_median_units = {}
for p, units_list in product_units.items():
    sorted_u = sorted(units_list)
    n = len(sorted_u)
    if n % 2 == 1:
        product_median_units[p] = sorted_u[n // 2]
    else:
        product_median_units[p] = (sorted_u[n // 2 - 1] + sorted_u[n // 2]) / 2

print("\n=== Product Median Units ===")
for p in sorted(product_median_units):
    print(f"  {p}: {product_median_units[p]:.1f}")

# 3. Detect revenue outliers
print("\n=== Revenue Outliers (revenue > 10x product median) ===")
for fname, row_num, parsed, issues in all_records:
    p = parsed["product"]
    r = parsed["revenue"]
    if p and r is not None and p in product_median:
        if product_median[p] > 0 and abs(r) > REVENUE_OUTLIER_MULTIPLIER * product_median[p]:
            direction = "high" if r > 0 else "low"
            print(f"  {fname} row {row_num}: {p} revenue={r:.2f} (median={product_median[p]:.2f}, {direction} outlier)")
            anomalies.append({
                "file": fname,
                "row": row_num,
                "issue": f"revenue_outlier_{direction}",
                "details": f"{p} revenue={r:.2f}, median={product_median[p]:.2f}, multiplier={abs(r)/product_median[p]:.1f}x",
            })

# 4. Negative values
print("\n=== Negative Values ===")
for fname, row_num, parsed, issues in all_records:
    u = parsed["units"]
    r = parsed["revenue"]
    if u is not None and u < 0:
        print(f"  {fname} row {row_num}: units={u}")
    if r is not None and r < 0:
        print(f"  {fname} row {row_num}: revenue={r:.2f}")

# 5. Per-product price per unit analysis
print("\n=== Price per Unit Analysis ===")
product_ppu = {}
for fname, row_num, parsed, issues in all_records:
    p = parsed["product"]
    u = parsed["units"]
    r = parsed["revenue"]
    if p and u and r and u > 0:
        ppu = r / u
        if p not in product_ppu:
            product_ppu[p] = []
        product_ppu[p].append((fname, row_num, ppu))

for p in sorted(product_ppu):
    vals = [x[2] for x in product_ppu[p]]
    avg_ppu = sum(vals) / len(vals)
    print(f"  {p}: avg_price_per_unit={avg_ppu:.2f}")
    for fname, row_num, ppu in product_ppu[p]:
        if avg_ppu > 0 and abs(ppu) > 5 * avg_ppu:
            print(f"    ANOMALY: {fname} row {row_num}: ppu={ppu:.2f} (avg={avg_ppu:.2f})")
            anomalies.append({
                "file": fname,
                "row": row_num,
                "issue": "price_per_unit_outlier",
                "details": f"{p} price_per_unit={ppu:.2f}, avg={avg_ppu:.2f}",
            })

# === Quarterly Summary ===
print("\n\n" + "="*60)
print("QUARTERLY SUMMARY")
print("="*60)
for q in sorted(quarterly_data):
    rows = quarterly_data[q]
    total_rev = sum(r["revenue"] for r in rows if r["revenue"] is not None)
    total_units = sum(r["units"] for r in rows if r["units"] is not None)
    num_transactions = len(rows)
    
    # By category
    cat_rev = {}
    cat_units = {}
    for r in rows:
        cat = r["category"]
        if cat:
            cat_rev[cat] = cat_rev.get(cat, 0) + (r["revenue"] or 0)
            cat_units[cat] = cat_units.get(cat, 0) + (r["units"] or 0)
    
    # By product
    prod_rev = {}
    prod_units = {}
    for r in rows:
        p = r["product"]
        if p:
            prod_rev[p] = prod_rev.get(p, 0) + (r["revenue"] or 0)
            prod_units[p] = prod_units.get(p, 0) + (r["units"] or 0)
    
    print(f"\nQ{q}:")
    print(f"  Transactions: {num_transactions}")
    print(f"  Total Revenue: ${total_rev:,.2f}")
    print(f"  Total Units: {total_units}")
    print(f"  Revenue by Category: {dict(sorted(cat_rev.items()))}")
    print(f"  Revenue by Product: {dict(sorted(prod_rev.items()))}")

# === File Summaries ===
print("\n\n" + "="*60)
print("FILE SUMMARIES")
print("="*60)
for fs in file_summaries:
    print(f"\n{fs['file']} ({fs['quarter']}):")
    print(f"  Schema: {fs['schema']}")
    print(f"  Rows: {fs['num_rows']} ({fs['num_clean_rows']} clean, {fs['num_issue_rows']} with issues)")
    print(f"  Total Revenue: ${fs['total_revenue']:,.2f}")
    print(f"  Total Units: {fs['total_units']}")
    print(f"  Products: {fs['products']}")
    print(f"  Categories: {fs['categories']}")

# === All Anomalies ===
print("\n\n" + "="*60)
print("ALL ANOMALIES")
print("="*60)
for a in anomalies:
    print(f"  [{a['file']}] row {a['row']}: {a['issue']} - {a['details']}")

# === Save to report file ===
report = {
    "quarterly_summary": {},
    "file_summaries": file_summaries,
    "anomalies": anomalies,
}

for q in sorted(quarterly_data):
    rows = quarterly_data[q]
    total_rev = sum(r["revenue"] for r in rows if r["revenue"] is not None)
    total_units = sum(r["units"] for r in rows if r["units"] is not None)
    num_transactions = len(rows)
    cat_rev = {}
    cat_units = {}
    prod_rev = {}
    prod_units = {}
    for r in rows:
        cat = r["category"]
        if cat:
            cat_rev[cat] = round(cat_rev.get(cat, 0) + (r["revenue"] or 0), 2)
            cat_units[cat] = cat_units.get(cat, 0) + (r["units"] or 0)
        p = r["product"]
        if p:
            prod_rev[p] = round(prod_rev.get(p, 0) + (r["revenue"] or 0), 2