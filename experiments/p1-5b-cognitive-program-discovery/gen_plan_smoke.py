#!/usr/bin/env python3
"""Generate plan-smoke.json — hand-authored golden Plan IR (test infrastructure
for the P1-5B v0.2 compiler pipeline; NEVER injected into LLM prompts)."""

import json

VALIDATE = '''import csv, json
path = "${item}"
with open(path, newline="", encoding="utf-8-sig") as f:
    reader = csv.reader(f)
    header = [h.strip() for h in next(reader)]
    rows = [r for r in reader]
issues = []
if any(len(r) != len(header) for r in rows):
    issues.append("field_count_mismatch")
for r in rows:
    for v in r:
        if v.strip() in ("", "NA", "N/A", "NULL", "null", "nan", "-"):
            issues.append("missing_value")
            break
print(json.dumps({"file": path, "row_count": len(rows), "has_issues": len(issues) > 0, "issues": issues}))'''

REPAIR = '''import csv, json
path = "${item}"
def repair_field_count(row, ncols):
    merged = []
    i = 0
    while i < len(row):
        cur = row[i].strip()
        if cur.startswith("$") and i + 1 < len(row) and row[i + 1].replace(".", "", 1).isdigit():
            merged.append(cur + "," + row[i + 1].strip())
            i += 2
        else:
            merged.append(cur)
            i += 1
    return merged[:ncols] if len(merged) >= ncols else merged + [""] * (ncols - len(merged))
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
print(json.dumps({"file": path, "currency_repairs": repaired}))'''

ANALYZE = '''import csv, json
from pathlib import Path
path = "${item}"
STANDARD = ["date", "product", "category", "units", "revenue"]
ALIASES = {"date": ["date", "transaction_date"], "product": ["product", "item_name"], "category": ["category", "item_category"], "units": ["units", "quantity"], "revenue": ["revenue", "sales"]}
MISSING = {"", "NA", "N/A", "NULL", "null", "nan", "-"}
def repair_field_count(row, ncols):
    merged = []
    i = 0
    while i < len(row):
        cur = row[i].strip()
        if cur.startswith("$") and i + 1 < len(row) and row[i + 1].replace(".", "", 1).isdigit():
            merged.append(cur + "," + row[i + 1].strip())
            i += 2
        else:
            merged.append(cur)
            i += 1
    return merged[:ncols] if len(merged) >= ncols else merged + [""] * (ncols - len(merged))
def parse_number(v):
    s = v.strip()
    if s in MISSING:
        return None
    try:
        return float(s)
    except ValueError:
        return None
def detect_outliers(revenues):
    nums = sorted(v for v in revenues if v is not None and v >= 0)
    if len(nums) < 4:
        return []
    median = nums[len(nums) // 2]
    threshold = max(median * 10.0, 100.0)
    return [i for i, v in enumerate(revenues) if v is not None and v > threshold]
currency_repairs = 0
with open(path, newline="", encoding="utf-8-sig") as f:
    reader = csv.reader(f)
    header = [h.strip() for h in next(reader)]
    rows = []
    for raw in reader:
        if len(raw) != len(header):
            fixed = repair_field_count(raw, len(header))
            if fixed != raw:
                currency_repairs += 1
            raw = fixed
        rows.append(raw)
mapping = {}
for idx, col in enumerate(header):
    norm = col.strip().lower()
    for std, aliases in ALIASES.items():
        if norm in aliases and std not in mapping:
            mapping[std] = idx
            break
issues = []
if currency_repairs > 0:
    issues.append("currency_formatting")
if len(mapping) < len(STANDARD):
    issues.append("column_name_drift")
normalized = []
for row in rows:
    rec = {}
    for std in STANDARD:
        idx = mapping.get(std)
        rec[std] = row[idx] if idx is not None and idx < len(row) else ""
    normalized.append(rec)
missing_count = 0
for rec in normalized:
    for std in STANDARD:
        if rec[std].strip() in MISSING:
            missing_count += 1
if missing_count > 0:
    issues.append("missing_value_NA")
normalized = [rec for rec in normalized if rec["units"].strip().upper() != "NULL" and rec["revenue"].strip().upper() != "NULL" and rec["date"].strip().upper() != "NULL"]
for rec in normalized:
    rec["revenue"] = rec["revenue"].replace("$", "").replace(",", "")
    rec["units"] = rec["units"].replace("$", "").replace(",", "")
seen = set()
deduped = []
for rec in normalized:
    key = tuple(rec[std] for std in STANDARD)
    if key not in seen:
        seen.add(key)
        deduped.append(rec)
if len(deduped) < len(normalized):
    issues.append("duplicate_rows")
normalized = deduped
has_negative = any((parse_number(r["units"]) or 0) < 0 or (parse_number(r["revenue"]) or 0) < 0 for r in normalized)
if has_negative:
    issues.append("negative_values")
revenues = [parse_number(rec["revenue"]) for rec in normalized]
outliers = detect_outliers(revenues)
if outliers:
    issues.append("extreme_outliers")
total_revenue = round(sum(v for v in revenues if v is not None), 2)
total_units = round(sum(v for v in (parse_number(r["units"]) for r in normalized) if v is not None), 2)
name = Path(path).name
display = "Q" + name[6:-4].upper() if len(name) > 6 and name[6].isdigit() else name
print(json.dumps({"file": name, "display_name": display, "row_count": len(normalized), "total_revenue": total_revenue, "total_units": total_units, "issues": issues, "currency_repairs": currency_repairs, "missing_count": missing_count, "outlier_rows": [{"row": i + 2, "product": normalized[i]["product"], "revenue": revenues[i]} for i in outliers], "negative_rows": []}))'''

MERGE = '''import json
raw = "${per_file_results}"
data = json.loads(raw)
results = []
for item in data:
    try:
        results.append(json.loads(item.get("stdout", "{}") or "{}"))
    except Exception:
        pass
lines = []
lines.append("# P1 Flagship CSV Quality Report")
lines.append("")
lines.append("## data_quality")
for r in results:
    lines.append("### " + r.get("display_name", "?") + " (" + r.get("file", "?") + ")")
    if r.get("issues"):
        lines.append("- Issues: " + str(len(r["issues"])))
        for issue in r["issues"]:
            lines.append("  - " + issue)
    else:
        lines.append("- Issues: 0 (clean)")
    lines.append("- Rows: " + str(r.get("row_count", 0)))
    lines.append("")
lines.append("## quarterly_summary")
for r in results:
    display = r.get("display_name", "?")
    q_name = display.rsplit(".", 1)[0].rsplit("_", 1)[-1] if "_" in display else display
    lines.append("- " + q_name + " revenue: {:.2f}".format(r.get("total_revenue", 0)))
    lines.append("- " + r.get("display_name", "?") + " units: " + str(r.get("total_units", 0)))
lines.append("")
lines.append("## anomalies")
found_anom = False
for r in results:
    for o in r.get("outlier_rows", []):
        found_anom = True
        lines.append("- " + r.get("file", "?") + " row " + str(o["row"]) + ": " + str(o.get("product", "?")) + " revenue {:.2f} (extreme outlier)".format(o["revenue"]))
    for n in r.get("negative_rows", []):
        found_anom = True
        lines.append("- " + r.get("file", "?") + " row " + str(n["row"]) + ": negative values")
if not found_anom:
    lines.append("- No anomalies detected")
lines.append("")
lines.append("## recovery_log")
for r in results:
    actions = []
    if r.get("currency_repairs", 0) > 0:
        actions.append("repaired " + str(r["currency_repairs"]) + " unquoted currency field(s)")
    if "duplicate_rows" in r.get("issues", []):
        actions.append("removed duplicate rows (kept first occurrence)")
    if r.get("missing_count", 0) > 0:
        actions.append("flagged " + str(r["missing_count"]) + " missing value(s); treated as 0 in revenue sums")
    if "column_name_drift" in r.get("issues", []):
        actions.append("aligned columns to standard schema via keyword mapping")
    lines.append("- " + r.get("file", "?") + ": " + ("; ".join(actions) if actions else "no repairs needed"))
lines.append("")
total_issues = sum(len(r.get("issues", [])) for r in results)
files_with_issues = sum(1 for r in results if r.get("issues"))
grand_total = round(sum(r.get("total_revenue", 0) for r in results), 2)
lines.append("## Aggregate")
lines.append("- Total files processed: " + str(len(results)))
lines.append("- Files with issues: " + str(files_with_issues))
lines.append("- Total issues found: " + str(total_issues))
lines.append("- Grand total revenue: {:.2f}".format(grand_total))
lines.append("")
print("\\n".join(lines))'''

plan = {
    "goal": "Analyze all quarterly sales CSV files: detect data-quality issues, repair recoverable issues, compute quarterly statistics, merge results, and generate a consolidated Markdown report with evidence log.",
    "steps": [
        {
            "name": "analyze_each",
            "kind": "foreach",
            "description": "process each declared input file",
            "over": "inputs",
            "output": {"name": "per_file_results", "typeName": "List<FileAnalysis>", "fields": []},
            "body": [
                {
                    "name": "validate_file",
                    "kind": "primitive",
                    "description": "load file and detect data-quality issues",
                    "capability": "execute_python",
                    "code": VALIDATE,
                    "inputBindings": [],
                    "output": {
                        "name": "validation_result",
                        "typeName": "ValidationResult",
                        "fields": [
                            {"name": "file", "typeName": "String"},
                            {"name": "row_count", "typeName": "Integer"},
                            {"name": "has_issues", "typeName": "Boolean"},
                            {"name": "issues", "typeName": "List"},
                        ],
                    },
                },
                {
                    "name": "fix_if_needed",
                    "kind": "conditional",
                    "description": "repair recoverable issues when validation found any",
                    "condition": "exists(validation_result)",
                    "body": [
                        {
                            "name": "repair_file",
                            "kind": "primitive",
                            "description": "repair unquoted currency field splits",
                            "capability": "execute_python",
                            "code": REPAIR,
                            "inputBindings": [],
                            "output": {
                                "name": "repaired_content",
                                "typeName": "RepairResult",
                                "fields": [
                                    {"name": "file", "typeName": "String"},
                                    {"name": "currency_repairs", "typeName": "Integer"},
                                ],
                            },
                        }
                    ],
                },
                {
                    "name": "analyze_file",
                    "kind": "retry",
                    "description": "compute statistics with retry on transient failures",
                    "capability": "execute_python",
                    "retry": {"maxAttempts": 3},
                    "code": ANALYZE,
                    "inputBindings": [],
                    "output": {
                        "name": "file_analysis",
                        "typeName": "FileAnalysis",
                        "fields": [
                            {"name": "file", "typeName": "String"},
                            {"name": "display_name", "typeName": "String"},
                            {"name": "row_count", "typeName": "Integer"},
                            {"name": "total_revenue", "typeName": "Number"},
                            {"name": "total_units", "typeName": "Number"},
                            {"name": "issues", "typeName": "List"},
                            {"name": "currency_repairs", "typeName": "Integer"},
                            {"name": "missing_count", "typeName": "Integer"},
                            {"name": "outlier_rows", "typeName": "List"},
                            {"name": "negative_rows", "typeName": "List"},
                        ],
                    },
                },
            ],
        },
        {
            "name": "merge_report",
            "kind": "primitive",
            "description": "merge per-file analyses into the consolidated report",
            "capability": "execute_python",
            "code": MERGE,
            "inputBindings": [
                {"param": "per_file_results", "source": "analyze_each", "binding": "per_file_results"}
            ],
            "output": {"name": "report_record", "typeName": "Report", "fields": []},
        },
        {
            "name": "write_report",
            "kind": "primitive",
            "description": "persist the consolidated report artifact",
            "capability": "write_file",
            "writePath": "p1_flagship_report.md",
            "code": None,
            "inputBindings": [
                {"param": "content", "source": "merge_report", "binding": "report_record"}
            ],
            "output": {
                "name": "report_ref",
                "typeName": "ArtifactRef",
                "fields": [
                    {"name": "id", "typeName": "String"},
                    {"name": "path", "typeName": "String"},
                ],
            },
        },
    ],
    "dataFlow": [
        {"fromStep": "analyze_each", "toStep": "merge_report", "binding": "per_file_results"},
        {"fromStep": "merge_report", "toStep": "write_report", "binding": "report_record"},
    ],
    "controlFlow": [
        {"step": "analyze_each", "kind": "foreach", "over": "inputs", "condition": None},
        {"step": "fix_if_needed", "kind": "conditional", "over": None, "condition": "exists(validation_result)"},
        {"step": "analyze_file", "kind": "retry", "over": None, "condition": None},
    ],
}

with open("experiments/p1-5b-cognitive-program-discovery/plan-smoke.json", "w", encoding="utf-8") as f:
    json.dump(plan, f, indent=2)
print("plan-smoke.json written")