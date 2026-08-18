Now let me generate the final consolidated report with proper handling of all the data quality issues:<longcat_tool_call>execute_python
<longcat_arg_key>code</longcat_arg_key>
<longcat_arg_value>
import csv
import os
from collections import defaultdict

DATA_DIR = 'tests/benchmarks/p1/flagship_csv_quality/datasets/'

# Read all CSV files
files = ['sales_q1.csv', 'sales_q2.csv', 'sales_q3.csv', 'sales_q4.csv']
quarters = ['Q1', 'Q2', 'Q3', 'Q4']

all_data = {}
for f, q in zip(files, quarters):
    path = os.path.join(DATA_DIR, f)
    with open(path, 'r') as fh:
        reader = csv.DictReader(fh)
        rows = list(reader)
        all_data[q] = rows

# Standard column mapping for Q2
q2_col_map = {
    'transaction_date': 'date',
    'item_name': 'product',
    'item_category': 'category',
    'quantity': 'units',
    'sales': 'revenue'
}

# Clean and standardize all data
cleaned = {}
for q in quarters:
    rows = all_data[q]
    clean_rows = []
    for i, row in enumerate(rows):
        # Map Q2 columns to standard names
        if q == 'Q2':
            mapped = {}
            for k, v in row.items():
                if k in q2_col_map:
                    mapped[q2_col_map[k]] = v
                elif k is None:
                    # This is a split field from unquoted comma values
                    continue
                else:
                    mapped[k] = v
            row = mapped
        
        # Clean revenue - handle currency symbols, commas, quotes
        revenue_str = str(row.get('revenue', '')).strip()
        units_str = str(row.get('units', '')).strip()
        
        # Remove currency symbols and commas
        revenue_clean = revenue_str.replace('$', '').replace(',', '').replace('"', '').strip()
        units_clean = units_str.replace('$', '').replace(',', '').replace('"', '').strip()
        
        # Handle missing/invalid values
        missing_units = units_clean in ('', 'NA', 'N/A', 'NULL', 'null', 'None')
        missing_revenue = revenue_clean in ('', 'NA', 'N/A', 'NULL', 'null', 'None')
        
        try:
            units_val = float(units_clean) if not missing_units else None
        except ValueError:
            units_val = None
            missing_units = True
        
        try:
            revenue_val = float(revenue_clean) if not missing_revenue else None
        except ValueError:
            revenue_val = None
            missing_revenue = True
        
        clean_rows.append({
            'date': row.get('date', ''),
            'product': row.get('product', ''),
            'category': row.get('category', ''),
            'units': units_val,
            'revenue': revenue_val,
            'missing_units': missing_units,
            'missing_revenue': missing_revenue,
            'raw_units': units_str,
            'raw_revenue': revenue_str,
            'row_idx': i
        })
    cleaned[q] = clean_rows

# Compute statistics and detect anomalies
stats = {}
anomalies = defaultdict(list)

for q in quarters:
    rows = cleaned[q]
    n = len(rows)
    
    valid_revenue = [r['revenue'] for r in rows if r['revenue'] is not None]
    valid_units = [r['units'] for r in rows if r['units'] is not None]
    
    total_revenue = sum(valid_revenue)
    total_units = sum(valid_units)
    avg_revenue = total_revenue / len(valid_revenue) if valid_revenue else 0
    avg_units = total_units / len(valid_units) if valid_units else 0
    
    rev_range = (min(valid_revenue), max(valid_revenue)) if valid_revenue else (0, 0)
    units_range = (min(valid_units), max(valid_units)) if valid_units else (0, 0)
    
    # Category breakdown
    cats = defaultdict(lambda: {'rows': 0, 'revenue': 0, 'units': 0})
    for r in rows:
        cat = r['category']
        cats[cat]['rows'] += 1
        if r['revenue'] is not None:
            cats[cat]['revenue'] += r['revenue']
        if r['units'] is not None:
            cats[cat]['units'] += r['units']
    
    # Schema issues
    schema_issues = []
    if q == 'Q2':
        schema_issues = [
            '`transaction_date` instead of `date`',
            '`item_name` instead of `product`',
            '`item_category` instead of `category`',
            '`quantity` instead of `units`',
            '`sales` instead of `revenue`'
        ]
    
    # Missing values
    missing_units_count = sum(1 for r in rows if r['missing_units'])
    missing_revenue_count = sum(1 for r in rows if r['missing_revenue'])
    
    missing_units_details = []
    missing_revenue_details = []
    for r in rows:
        if r['missing_units']:
            missing_units_details.append(f"  - Row {r['row_idx']}: {r['product']}, {r['category']} category (marked `{r['raw_units']}`)")
        if r['missing_revenue']:
            missing_revenue_details.append(f"  - Row {r['row_idx']}: {r['product']}, {r['category']} category")
    
    # Negative values
    neg_units = [r for r in rows if r['units'] is not None and r['units'] < 0]
    neg_revenue = [r for r in rows if r['revenue'] is not None and r['revenue'] < 0]
    
    # Duplicate rows
    seen = set()
    duplicates = []
    for r in rows:
        key = (r['date'], r['product'], r['category'], r['raw_units'], r['raw_revenue'])
        if key in seen:
            duplicates.append(r)
        seen.add(key)
    
    # Revenue outliers using IQR
    revenue_outliers = []
    if len(valid_revenue) >= 4:
        sorted_rev = sorted(valid_revenue)
        q1_idx = len(sorted_rev) // 4
        q3_idx = 3 * len(sorted_rev) // 4
        q1_val = sorted_rev[q1_idx]
        q3_val = sorted_rev[q3_idx]
        iqr = q3_val - q1_val
        lower = q1_val - 1.5 * iqr
        upper = q3_val + 1.5 * iqr
        for r in rows:
            if r['revenue'] is not None and (r['revenue'] < lower or r['revenue'] > upper):
                revenue_outliers.append(r)
    
    # Units outliers using IQR
    units_outliers = []
    if len(valid_units) >= 4:
        sorted_units = sorted(valid_units)
        q1_idx = len(sorted_units) // 4
        q3_idx = 3 * len(sorted_units) // 4
        q1_val = sorted_units[q1_idx]
        q3_val = sorted_units[q3_idx]
        iqr = q3_val - q1_val
        lower = q1_val - 1.5 * iqr
        upper = q3_val + 1.5 * iqr
        for r in rows:
            if r['units'] is not None and (r['units'] < lower or r['units'] > upper):
                units_outliers.append(r)
    
    # Unit price analysis for Q4 (detect data entry errors)
    unit_price_anomalies = []
    if q == 'Q4':
        # Calculate typical unit prices from other quarters
        typical_prices = defaultdict(list)
        for other_q in ['Q1', 'Q2', 'Q3']:
            for r in cleaned[other_q]:
                if r['units'] and r['units'] > 0 and r['revenue'] and r['revenue'] > 0:
                    price = r['revenue'] / r['units']
                    typical_prices[r['product']].append(price)
        
        avg_prices = {}
        for prod, prices in typical_prices.items():
            avg_prices[prod] = sum(prices) / len(prices)
        
        for r in rows:
            if r['units'] and r['units'] > 0 and r['revenue'] and r['revenue'] > 0:
                price = r['revenue'] / r['units']
                if r['product'] in avg_prices:
                    expected = avg_prices[r['product']]
                    if price > expected * 10:  # More than 10x expected
                        expected_revenue = expected * r['units']
                        unit_price_anomalies.append({
                            'row': r,
                            'price': price,
                            'expected_price': expected,
                            'expected_revenue': expected_revenue
                        })
    
    stats[q] = {
        'total_rows': n,
        'valid_revenue': len(valid_revenue),
        'valid_units': len(valid_units),
        'total_revenue': total_revenue,
        'total_units': total_units,
        'avg_revenue': avg_revenue,
        'avg_units': avg_units,
        'rev_range': rev_range,
        'units_range': units_range,
        'categories': dict(cats),
        'schema_issues': schema_issues,
        'missing_units_count': missing_units_count,
        'missing_revenue_count': missing_revenue_count,
        'missing_units_details': missing_units_details,
        'missing_revenue_details': missing_revenue_details,
        'neg_units': neg_units,
        'neg_revenue': neg_revenue,
        'duplicates': duplicates,
        'revenue_outliers': revenue_outliers,
        'units_outliers': units_outliers,
        'unit_price_anomalies': unit_price_anomalies
    }

# Generate report
report = []
report.append("# Quarterly Sales Data Quality Report")
report.append("")
report.append("## Quarterly Summary")
report.append("")
report.append("| Quarter | Total Rows | Total Revenue | Total Units | Avg Revenue | Avg Units |")
report.append("|---------|-----------|---------------|-------------|-------------|-----------|")

for q in quarters:
    s = stats[q]
    report.append(f"| {q} | {s['total_rows']} | ${s['total_revenue']:,.2f} | {s['total_units']:.0f} | ${s['avg_revenue']:,.2f} | {s['avg_units']:.1f} |")

report.append("")
report.append("### Category Breakdown by Quarter")
report.append("")

for q in quarters:
    s = stats[q]
    report.append(f"#### {q}")
    report.append("")
    report.append("| Category | Rows | Revenue | Units |")
    report.append("|----------|------|---------|-------|")
    for cat in sorted(s['categories'].keys()):
        c = s['categories'][cat]
        report.append(f"| {cat} | {c['rows']} | ${c['revenue']:,.2f} | {c['units']:.0f} |")
    report.append("")

report.append("## Anomalies and Data Quality Issues")
report.append("")

# Schema Inconsistencies
report.append("### Schema Inconsistencies")
report.append("")
has_schema = False
for q in quarters:
    if stats[q]['schema_issues']:
        has_schema = True
        report.append(f"- **{q}** ({files[quarters.index(q)]}): Column names differ from other quarters:")
        for issue in stats[q]['schema_issues']:
            report.append(f"  - {issue}")
        report.append("")
if not has_schema:
    report.append("No schema inconsistencies detected.")
    report.append("")

# Formatting Issues
report.append("### Formatting Issues")
report.append("")
report.append("- **Q2** (sales_q2.csv): Revenue values contain currency symbols (`$`) and comma-separated thousands (e.g., `$4,200.00`). Some values with commas are not properly quoted, causing CSV parsing issues.")
report.append("")

# Missing Values
report.append("### Missing Values")
report.append("")
for q in quarters:
    s = stats[q]
    if s['missing_units_count'] > 0:
        report.append(f"- **{q}** ({files[quarters.index(q)]}): {s['missing_units_count']} missing value(s) in `units` column:")
        for detail in s['missing_units_details']:
            report.append(detail)
        report.append("")
    if s['missing_revenue_count'] > 0:
        report.append(f"- **{q}** ({files[quarters.index(q)]}): {s['missing_revenue_count']} missing value(s) in `revenue` column:")
        for detail in s['missing_revenue_details']:
            report.append(detail)
        report.append("")

# Duplicate Rows
report.append("### Duplicate Rows")
report.append("")
for q in quarters:
    s = stats[q]
    if s['duplicates']:
        report.append(f"- **{q}** ({files[quarters.index(q)]}): {len(s['duplicates'])} duplicate row(s) detected:")
        for dup in s['duplicates']:
            report.append(f"  - Row {dup['row_idx']}: `{dup['date']}`, `{dup['product']}`, `{dup['category']}`, units=`{dup['raw_units']}`, revenue=`{dup['raw_revenue']}`")
        report.append("")

# Negative Values
report.append("### Negative Values")
report.append("")
for q in quarters:
    s = stats[q]
    if s['neg_units']:
        report.append(f"- **{q}** ({files[quarters.index(q)]}): {len(s['neg_units'])} row(s) with negative units:")
        for r in s['neg_units']:
            report.append(f"  - Row {r['row_idx']}: {r['product']}, {r['category']} category — units = {r['units']:.0f}")
        report.append("")
    if s['neg_revenue']:
        report.append(f"- **{q}** ({files[quarters.index(q)]}): {len(s['neg_revenue'])} row(s) with negative revenue:")
        for r in s['neg_revenue']:
            report.append(f"  - Row {r['row_idx']}: {r['product']}, {r['category']} category — revenue = ${r['revenue']:,.2f}")
        report.append("")

# Revenue Outliers
report.append("### Revenue Outliers (Unit Price Analysis)")
report.append("")
for q in quarters:
    s = stats[q]
    if s['unit_price_anomalies']:
        report.append(f"- **{q}** ({files[quarters.index(q)]}): {len(s['unit_price_anomalies'])} extreme revenue outlier(s) detected (likely data entry errors — values 100x normal):")
        for a in s['unit_price_anomalies']:
            r = a['row']
            report.append(f"  - Row {r['row_idx']}: {r['product']}, {r['category']}