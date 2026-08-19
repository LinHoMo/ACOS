# P1 Flagship CSV Quality Report

## data_quality

### Q1 (sales_q1.csv)
- Issues: 0 (clean)
- Rows: 7

### Q2 (sales_q2.csv)
- Issues: 2
  - currency_formatting
  - missing_value_NA
- Rows: 6

### Q3 (sales_q3.csv)
- Issues: 3
  - missing_value_NA
  - missing_value_NULL
  - duplicate_rows
- Rows: 5

### Q4 (sales_q4.csv)
- Issues: 2
  - negative_values
  - extreme_outliers
- Rows: 7

## quarterly_summary

- Q1 revenue: 33850.00
- Q1 units: 665.0
- Q2 revenue: 24250.00
- Q2 units: 410.0
- Q3 revenue: 22500.00
- Q3 units: 325.0
- Q4 revenue: 2118550.00
- Q4 units: 573.0

## anomalies

- sales_q4.csv row 4: Gadget X revenue 900000.00 (extreme outlier)
- sales_q4.csv row 8: Widget A revenue 1200000.00 (extreme outlier)
- sales_q4.csv row 3: Widget B negative values (units=-5.0, revenue=-250.0)

## recovery_log

- sales_q1.csv: no repairs needed
- sales_q2.csv: repaired 2 unquoted currency field(s); flagged 1 missing value(s); treated as 0 in revenue sums
- sales_q3.csv: removed duplicate rows (kept first occurrence); flagged 4 missing value(s); treated as 0 in revenue sums
- sales_q4.csv: no repairs needed

## Aggregate

- Total files processed: 4
- Files with issues: 3
- Total issues found: 7
- Grand total revenue: 2199150.00

## Evidence Log

- sales_q1.csv: loaded, schema-aligned, validated (0 issues), repaired, revalidated, statistics computed
- sales_q2.csv: loaded, schema-aligned, validated (2 issues), repaired, revalidated, statistics computed
- sales_q3.csv: loaded, schema-aligned, validated (3 issues), repaired, revalidated, statistics computed
- sales_q4.csv: loaded, schema-aligned, validated (2 issues), repaired, revalidated, statistics computed
- aggregate: totals computed from per-file statistics
