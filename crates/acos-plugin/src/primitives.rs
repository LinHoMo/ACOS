//! The five built-in MVP cognitive primitives.

use async_trait::async_trait;
use serde_json::Value;

use acos_core::error::AcosError;
use acos_core::traits::{CapabilityDesc, Primitive};
use acos_core::types::{EffectDecl, EffectKind, FailureClass};
use acos_core::types::{TypedValue, ValueType};

/// `search` —network read; returns an empty document list for MVP.
pub struct SearchPrimitive;

impl std::fmt::Debug for SearchPrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SearchPrimitive")
    }
}

#[async_trait]
impl Primitive for SearchPrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "search".into(),
            name: "Search".into(),
            input_type: "SearchQuery".into(),
            output_type: "DocumentList".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![EffectDecl {
            kind: EffectKind::NetworkRead,
            description: "network read".into(),
            reversible: true,
        }]
    }

    async fn invoke(&self, _input: TypedValue) -> Result<TypedValue, AcosError> {
        Ok(TypedValue {
            value_type: ValueType::List,
            payload: serde_json::json!([]),
        })
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(&self, _effect: &EffectDecl, _input: TypedValue) -> Result<(), AcosError> {
        Ok(())
    }
}

/// `read_file` —filesystem read.
pub struct ReadFilePrimitive;

impl std::fmt::Debug for ReadFilePrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ReadFilePrimitive")
    }
}

#[async_trait]
impl Primitive for ReadFilePrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "read_file".into(),
            name: "Read File".into(),
            input_type: "FileRef".into(),
            output_type: "Document".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![EffectDecl {
            kind: EffectKind::FsRead,
            description: "filesystem read".into(),
            reversible: true,
        }]
    }

    async fn invoke(&self, input: TypedValue) -> Result<TypedValue, AcosError> {
        let path = input
            .payload
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AcosError::PrimitiveFailure {
                message: "read_file requires a 'path' string".into(),
                primitive_id: Some("read_file".into()),
                class: FailureClass::Unknown,
            })?;

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| AcosError::PrimitiveFailure {
                message: format!("failed to read {path}: {e}"),
                primitive_id: Some("read_file".into()),
                class: FailureClass::Unknown,
            })?;

        Ok(TypedValue {
            value_type: ValueType::Scalar,
            payload: serde_json::json!({ "path": path, "content": content }),
        })
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(&self, _effect: &EffectDecl, _input: TypedValue) -> Result<(), AcosError> {
        Ok(())
    }
}

/// `write_file` —filesystem write (with delete compensation).
pub struct WriteFilePrimitive;

impl std::fmt::Debug for WriteFilePrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WriteFilePrimitive")
    }
}

#[async_trait]
impl Primitive for WriteFilePrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "write_file".into(),
            name: "Write File".into(),
            input_type: "ArtifactWriteRequest".into(),
            output_type: "ArtifactRef".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![EffectDecl {
            kind: EffectKind::FsWrite,
            description: "filesystem write".into(),
            reversible: true,
        }]
    }

    async fn invoke(&self, input: TypedValue) -> Result<TypedValue, AcosError> {
        let path = input
            .payload
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AcosError::PrimitiveFailure {
                message: "write_file requires a 'path' string".into(),
                primitive_id: Some("write_file".into()),
                class: FailureClass::Unknown,
            })?;

        let content = input
            .payload
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if let Some(parent) = std::path::Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }

        tokio::fs::write(path, content.as_bytes())
            .await
            .map_err(|e| AcosError::PrimitiveFailure {
                message: format!("failed to write {path}: {e}"),
                primitive_id: Some("write_file".into()),
                class: FailureClass::Unknown,
            })?;

        Ok(TypedValue {
            value_type: ValueType::Scalar,
            payload: serde_json::json!({ "path": path }),
        })
    }

    fn has_compensation(&self, effect: &EffectDecl) -> bool {
        effect.kind == EffectKind::FsWrite
    }

    async fn compensate(&self, effect: &EffectDecl, input: TypedValue) -> Result<(), AcosError> {
        if effect.kind != EffectKind::FsWrite {
            return Ok(());
        }
        if let Some(path) = input.payload.get("path").and_then(Value::as_str) {
            tokio::fs::remove_file(path).await.ok();
        }
        Ok(())
    }
}

/// `execute_python` —process execution of Python code (process spawn).
pub struct ExecutePythonPrimitive;

impl std::fmt::Debug for ExecutePythonPrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExecutePythonPrimitive")
    }
}

#[async_trait]
impl Primitive for ExecutePythonPrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "execute_python".into(),
            name: "Execute Python".into(),
            input_type: "PythonExecutionRequest".into(),
            output_type: "ExecutionResult".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![
            EffectDecl {
                kind: EffectKind::ProcessSpawn,
                description: "process execution".into(),
                reversible: true,
            },
            EffectDecl {
                kind: EffectKind::FsRead,
                description: "optional filesystem/network access".into(),
                reversible: true,
            },
        ]
    }

    async fn invoke(&self, input: TypedValue) -> Result<TypedValue, AcosError> {
        let code = input
            .payload
            .get("code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AcosError::PrimitiveFailure {
                message: "execute_python requires a 'code' string".into(),
                primitive_id: Some("execute_python".into()),
                class: FailureClass::Unknown,
            })?;

        let python = ["python3", "python", "py"]
            .iter()
            .find(|cmd| which(cmd))
            .copied()
            .ok_or_else(|| AcosError::ProviderFailure {
                message: "no python interpreter found on PATH".into(),
                provider: "execute_python".into(),
            })?;

        let mut command = tokio::process::Command::new(python);
        command.arg("-c");
        let mut run_code = code.to_string();
        let mut temp_inputs_path: Option<std::path::PathBuf> = None;

        // P1-5B v0.4 S2/S3: structured inputs transport. When the harness
        // enables ACOS_STRUCTURED_INPUTS, the node's bound inputs (everything
        // except `code`) are injected as a real Python `inputs` dict — values
        // cross stage boundaries as structured data, not source-code
        // interpolation. Env-var transport with a temp-file fallback for
        // payloads that would exceed the process environment block limit.
        if std::env::var("ACOS_STRUCTURED_INPUTS").as_deref() == Ok("1") {
            let mut inputs_map = serde_json::Map::new();
            if let Some(obj) = input.payload.as_object() {
                for (k, v) in obj {
                    if k != "code" {
                        inputs_map.insert(k.clone(), v.clone());
                    }
                }
            }
            let inputs_json =
                serde_json::to_string(&serde_json::Value::Object(inputs_map)).unwrap_or_else(|_| "{}".into());
            if inputs_json.len() > 4000 {
                let path = std::env::temp_dir().join(format!("acos_inputs_{}.json", std::process::id()));
                std::fs::write(&path, &inputs_json).map_err(|e| AcosError::PrimitiveFailure {
                    message: format!("failed to stage structured inputs: {e}"),
                    primitive_id: Some("execute_python".into()),
                    class: FailureClass::Unknown,
                })?;
                temp_inputs_path = Some(path.clone());
                command.env("ACOS_PYTHON_INPUTS_PATH", &path);
            } else {
                command.env("ACOS_PYTHON_INPUTS", &inputs_json);
            }
            let prologue = "import os, json\n\
                if os.environ.get('ACOS_PYTHON_INPUTS_PATH'):\n\
                \x20   _acos_f = open(os.environ['ACOS_PYTHON_INPUTS_PATH'], encoding='utf-8')\n\
                \x20   inputs = json.load(_acos_f)\n\
                \x20   _acos_f.close()\n\
                else:\n\
                \x20   inputs = json.loads(os.environ.get('ACOS_PYTHON_INPUTS', '{}'))\n";
            run_code = format!("{prologue}\n{code}");
        }
        command.arg(&run_code);

        let output = command
            .output()
            .await
            .map_err(|e| AcosError::PrimitiveFailure {
                message: format!("failed to spawn python: {e}"),
                primitive_id: Some("execute_python".into()),
                class: FailureClass::Unknown,
            })?;

        if let Some(path) = temp_inputs_path {
            std::fs::remove_file(path).ok();
        }

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if !output.status.success() {
            return Err(AcosError::PrimitiveFailure {
                message: format!("python exited {:?}: {stderr}", output.status.code()),
                primitive_id: Some("execute_python".into()),
                class: FailureClass::Unknown,
            });
        }

        Ok(TypedValue {
            value_type: ValueType::Scalar,
            payload: serde_json::json!({ "stdout": stdout, "stderr": stderr }),
        })
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(&self, _effect: &EffectDecl, _input: TypedValue) -> Result<(), AcosError> {
        Ok(())
    }
}

/// `csv.inspect_schema` —deterministic schema inspection for CSV files.
///
/// P1-5B v0.3 (Capability Contract & Typed Execution): a High-Level Cognitive
/// Capability. The model writes structured parameters, not Python:
/// `code: {"path": "${item}"}`. Returns a `{stdout, stderr}` envelope whose
/// stdout is the JSON schema report —same envelope semantics as
/// `execute_python`, so the Plan compiler is untouched.
pub struct CsvInspectSchemaPrimitive;

impl std::fmt::Debug for CsvInspectSchemaPrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CsvInspectSchemaPrimitive")
    }
}

#[async_trait]
impl Primitive for CsvInspectSchemaPrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "csv.inspect_schema".into(),
            name: "CSV Inspect Schema".into(),
            input_type: "CsvInspectRequest".into(),
            output_type: "CsvSchemaReport".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![EffectDecl {
            kind: EffectKind::FsRead,
            description: "read CSV file".into(),
            reversible: true,
        }]
    }

    async fn invoke(&self, input: TypedValue) -> Result<TypedValue, AcosError> {
        let params = parse_params(&input)?;
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AcosError::PrimitiveFailure {
                message: "csv.inspect_schema requires a 'path' string".into(),
                primitive_id: Some("csv.inspect_schema".into()),
                class: FailureClass::Unknown,
            })?;
        let (header, rows) = read_csv(path).await?;
        let row_count = rows.len();
        let ncols = header.len();
        let mut column_types: Vec<serde_json::Value> = Vec::with_capacity(ncols);
        for (i, name) in header.iter().enumerate() {
            let col: Vec<&str> = rows
                .iter()
                .filter_map(|r| r.get(i).map(|v| v.as_str()))
                .collect();
            let ty = if !col.is_empty() && col.iter().all(|v| v.parse::<i64>().is_ok()) {
                "integer"
            } else if !col.is_empty() && col.iter().all(|v| parse_number(v).is_some()) {
                "number"
            } else {
                "string"
            };
            column_types.push(serde_json::json!({ "name": name, "type": ty }));
        }
        let mut issues: Vec<&str> = Vec::new();
        if rows.iter().any(|r| r.len() != ncols) {
            issues.push("field_count_mismatch");
        }
        if rows.iter().flatten().any(|v| is_missing(v)) {
            issues.push("missing_value");
        }
        let report = serde_json::json!({
            "columns": column_types,
            "row_count": row_count,
            "issues": issues,
        });
        Ok(envelope(&report))
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(&self, _effect: &EffectDecl, _input: TypedValue) -> Result<(), AcosError> {
        Ok(())
    }
}

/// `csv.aggregate` —deterministic column aggregation with **runtime schema
/// enforcement** (P1-5B v0.3 experiment C).
///
/// Parameters: `code: {"path": "${item}", "columns": ["revenue", "units"]}`.
/// Every referenced column is validated against the file's actual header; an
/// unknown column is rejected with a `PrimitiveFailure` (the primitive
/// enforces the schema —the model cannot silently hallucinate column names).
///
/// Canonical aggregation semantics: unquoted currency splits are merged
/// (`$3` + `150.00` -> `$3,150.00`); rows with a literal `NULL` value in any
/// column are dropped (N/A / empty cells are kept and sum as 0); fully
/// duplicate rows (all columns) keep the first occurrence; remaining values
/// are summed with negatives included. The result reports `row_count` (kept
/// rows), `dropped_rows`, and per-column `sum`.
pub struct CsvAggregatePrimitive;

impl std::fmt::Debug for CsvAggregatePrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CsvAggregatePrimitive")
    }
}

#[async_trait]
impl Primitive for CsvAggregatePrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "csv.aggregate".into(),
            name: "CSV Aggregate".into(),
            input_type: "CsvAggregateRequest".into(),
            output_type: "CsvAggregateResult".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![EffectDecl {
            kind: EffectKind::FsRead,
            description: "read CSV file".into(),
            reversible: true,
        }]
    }

    async fn invoke(&self, input: TypedValue) -> Result<TypedValue, AcosError> {
        let params = parse_params(&input)?;
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AcosError::PrimitiveFailure {
                message: "csv.aggregate requires a 'path' string".into(),
                primitive_id: Some("csv.aggregate".into()),
                class: FailureClass::Unknown,
            })?;
        let columns: Vec<String> = parse_columns(params.get("columns"))
            .ok_or_else(|| AcosError::PrimitiveFailure {
                message: "csv.aggregate requires a non-empty 'columns' array".into(),
                primitive_id: Some("csv.aggregate".into()),
                class: FailureClass::Unknown,
            })?;
        if columns.is_empty() {
            return Err(AcosError::PrimitiveFailure {
                message: "csv.aggregate requires a non-empty 'columns' array".into(),
                primitive_id: Some("csv.aggregate".into()),
                class: FailureClass::Unknown,
            });
        }
        let (header, rows) = read_csv(path).await?;
        // Runtime schema enforcement: reject unknown column references.
        let missing: Vec<&str> = columns
            .iter()
            .map(|c| c.as_str())
            .filter(|c| !header.iter().any(|h| h == *c))
            .collect();
        if !missing.is_empty() {
            return Err(AcosError::PrimitiveFailure {
                message: format!(
                    "csv.aggregate: unknown column(s) {:?}; actual header is {:?}",
                    missing, header
                ),
                primitive_id: Some("csv.aggregate".into()),
                class: FailureClass::Unknown,
            });
        }
        let idxs: Vec<usize> = columns
            .iter()
            .map(|c| header.iter().position(|h| h == c).unwrap_or(0))
            .collect();
        // Canonical deterministic aggregation semantics (mirrors the flagship
        // benchmark's ground truth):
        //   1. unquoted currency splits are merged (`$3` + `150.00` -> `$3,150.00`)
        //   2. rows with a literal `NULL` value in ANY column are dropped
        //      (N/A / empty cells are kept and sum as 0)
        //   3. fully duplicate rows (all columns) keep the first occurrence
        //   4. remaining values are summed; negatives included
        let mut kept: Vec<Vec<String>> = Vec::new();
        let mut seen: std::collections::HashSet<Vec<String>> = std::collections::HashSet::new();
        let mut null_dropped = 0usize;
        let mut dup_dropped = 0usize;
        let mut currency_merges = 0usize;
        let mut negative_values = false;
        for raw in &rows {
            let mut r: Vec<String> = Vec::with_capacity(raw.len());
            let mut i = 0usize;
            while i < raw.len() {
                let c = raw[i].trim().to_string();
                if c.starts_with('$')
                    && i + 1 < raw.len()
                    && raw[i + 1]
                        .trim()
                        .replace(['.', ','], "")
                        .parse::<f64>()
                        .is_ok()
                {
                    r.push(c + "," + raw[i + 1].trim());
                    i += 2;
                    currency_merges += 1;
                } else {
                    r.push(c);
                    i += 1;
                }
            }
            if r.iter().any(|v| v.to_uppercase() == "NULL") {
                null_dropped += 1;
                continue;
            }
            if seen.insert(r.clone()) {
                kept.push(r);
            } else {
                dup_dropped += 1;
            }
        }
        let mut issues: Vec<&str> = Vec::new();
        if currency_merges > 0 {
            issues.push("currency_formatting");
        }
        if null_dropped > 0 {
            issues.push("missing_value_NULL");
        }
        if dup_dropped > 0 {
            issues.push("duplicate_rows");
        }
        let mut sums: Vec<serde_json::Value> = Vec::with_capacity(columns.len());
        for (ci, col) in columns.iter().enumerate() {
            let mut sum = 0.0f64;
            for r in &kept {
                if let Some(v) = r.get(idxs[ci]) {
                    sum += parse_number(v).unwrap_or(0.0);
                    if parse_number(v).is_some_and(|n| n < 0.0) {
                        negative_values = true;
                    }
                }
            }
            sums.push(serde_json::json!({ "name": col, "sum": sum }));
        }
        if negative_values {
            issues.push("negative_values");
        }
        let result = serde_json::json!({
            "file": std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path),
            "row_count": kept.len(),
            "dropped_rows": null_dropped + dup_dropped,
            "currency_merges": currency_merges,
            "issues": issues,
            "columns": sums,
        });
        Ok(envelope(&result))
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(&self, _effect: &EffectDecl, _input: TypedValue) -> Result<(), AcosError> {
        Ok(())
    }
}

/// Extracts the referenced column names from a `columns` parameter.
///
/// Accepted shapes (all produced by the Plan IR / runtime):
/// - a JSON array of strings: `["revenue", "units"]`;
/// - a JSON array of objects: `[{"name": "revenue", "type": "number"}]`
///   (directly taken from a `csv.inspect_schema` report);
/// - a `csv.inspect_schema` envelope `{stdout: <report json>, stderr}` bound
///   via `inputBindings` — the report's `columns` entries are used.
fn parse_columns(v: Option<&Value>) -> Option<Vec<String>> {
    let v = v?;
    let names: Vec<String> = match v {
        Value::Array(a) => a
            .iter()
            .filter_map(col_name)
            .collect(),
        Value::String(s) => serde_json::from_str::<Value>(s)
            .ok()
            .and_then(|x| x.as_array().cloned())
            .map(|a| a.iter().filter_map(col_name).collect())
            .unwrap_or_default(),
        Value::Object(m) => m
            .get("stdout")
            .and_then(|s| s.as_str())
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .and_then(|report| report.get("columns").and_then(|c| c.as_array().cloned()))
            .map(|a| a.iter().filter_map(col_name).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    if names.is_empty() {
        None
    } else {
        Some(names)
    }
}

/// Column-name extractor: a string entry, or an object with a `name` field.
fn col_name(e: &Value) -> Option<String> {
    match e {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o.get("name").and_then(|n| n.as_str()).map(|n| n.to_string()),
        _ => None,
    }
}

/// Parses the structured parameters for a CSV primitive.
///
/// Two shapes are accepted (both produced by the same Plan `code` JSON):
/// - the parameters as a direct JSON object payload — the runtime's
///   `resolve_value` auto-parses a JSON-parseable `code` string, so
///   `{"path": "${item}"}` arrives already unwrapped;
/// - the `{"code": "<json string>"}` wrapper (direct `Primitive::invoke`
///   callers / unit tests).
fn parse_params(input: &TypedValue) -> Result<serde_json::Map<String, Value>, AcosError> {
    if let Some(obj) = input.payload.as_object() {
        // The runtime's `resolve_value` parses a JSON-parseable `code` string
        // into an object, so `{"code": {"path": ...}, ...}` is the common
        // shape; the `code`-nested object holds the parameters. Sibling
        // inputs (e.g. a `columns` inputBinding) are merged on top.
        let mut params = serde_json::Map::new();
        if let Some(code) = obj.get("code") {
            let nested = match code {
                Value::Object(o) => Some(o.clone()),
                Value::String(s) => serde_json::from_str::<Value>(s)
                    .ok()
                    .and_then(|v| v.as_object().cloned()),
                _ => None,
            };
            if let Some(n) = nested {
                params = n;
            }
        }
        for (k, v) in obj {
            if k != "code" {
                params.insert(k.clone(), v.clone());
            }
        }
        // Direct-object form (no `code` key): the payload itself is the params.
        if params.contains_key("path") || params.contains_key("columns") {
            return Ok(params);
        }
    }
    Err(AcosError::PrimitiveFailure {
        message: "csv primitive requires parameters (a JSON object with 'path')".into(),
        primitive_id: Some("csv".into()),
        class: FailureClass::Unknown,
    })
}

/// Wraps a value in the `{stdout, stderr}` envelope (runtime envelope
/// semantics shared with `execute_python`).
fn envelope(value: &Value) -> TypedValue {
    TypedValue {
        value_type: ValueType::Scalar,
        payload: serde_json::json!({
            "stdout": serde_json::to_string(value).unwrap_or_default(),
            "stderr": "",
        }),
    }
}

/// Reads a CSV file (utf-8-sig) into (header, rows) of raw cell strings.
/// Minimal quote-aware parsing: `"` toggles quoting; commas inside quotes are
/// kept; a field may contain a quoted comma (e.g. `"$1,200"`).
async fn read_csv(path: &str) -> Result<(Vec<String>, Vec<Vec<String>>), AcosError> {
    let raw = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| AcosError::PrimitiveFailure {
            message: format!("failed to read {path}: {e}"),
            primitive_id: Some("csv".into()),
            class: FailureClass::Unknown,
        })?;
    let text = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    let mut lines = text.lines();
    let header = parse_csv_line(lines.next().unwrap_or(""));
    let rows: Vec<Vec<String>> = lines
        .filter(|l| !l.trim().is_empty())
        .map(parse_csv_line)
        .collect();
    Ok((header, rows))
}

/// Parses one CSV line into fields, honoring double-quoted segments.
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for c in line.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                fields.push(cur.trim().to_string());
                cur = String::new();
            }
            _ => cur.push(c),
        }
    }
    fields.push(cur.trim().to_string());
    fields
}

/// Loose numeric parse: strips `$` and thousands separators, treats MISSING
/// sentinels as `None`.
fn parse_number(v: &str) -> Option<f64> {
    let s = v.trim();
    if is_missing(s) {
        return None;
    }
    let cleaned = s.replace(['$', ','], "");
    cleaned.parse::<f64>().ok()
}

/// MISSING-value sentinels (mirrors the flagship benchmark's semantics).
fn is_missing(v: &str) -> bool {
    matches!(
        v.trim(),
        "" | "NA" | "N/A" | "NULL" | "null" | "nan" | "-"
    )
}

/// `summarize` —text summarization backed by Claude (via LongCat).
///
/// When the `LONGCAT_API_KEY` (or `ANTHROPIC_API_KEY`) environment variable is
/// set, this primitive sends the document to Claude and returns a real
/// summary. Otherwise it falls back to a deterministic local summary so the
/// runtime still works offline.
pub struct SummarizePrimitive {
    llm: Option<acos_llm::LongCatClient>,
}

impl Default for SummarizePrimitive {
    fn default() -> Self {
        let llm = acos_llm::LongCatClient::from_env().ok();
        Self { llm }
    }
}

impl SummarizePrimitive {
    /// Creates a summarize primitive, using an LLM if configured.
    pub fn new() -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for SummarizePrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.llm {
            Some(client) => write!(f, "SummarizePrimitive(llm={})", client.model()),
            None => write!(f, "SummarizePrimitive(local)"),
        }
    }
}

#[async_trait]
impl Primitive for SummarizePrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "summarize".into(),
            name: "Summarize".into(),
            input_type: "Document".into(),
            output_type: "Summary".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![]
    }

    async fn invoke(&self, input: TypedValue) -> Result<TypedValue, AcosError> {
        let text = extract_text(&input);

        let summary = match &self.llm {
            Some(llm) => llm_summarize(llm, &text).await?,
            None => summarize_text(&text),
        };

        Ok(TypedValue {
            value_type: ValueType::Scalar,
            payload: serde_json::json!({ "summary": summary }),
        })
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(&self, _effect: &EffectDecl, _input: TypedValue) -> Result<(), AcosError> {
        Ok(())
    }
}

/// Calls Claude to produce a concise summary of the given text.
async fn llm_summarize(llm: &acos_llm::LongCatClient, text: &str) -> Result<String, AcosError> {
    let system = "You are a concise summarizer. Produce a clear, accurate summary of the given text in Chinese. Be specific; do not invent facts. Keep it to 2-4 sentences unless the input is long.";
    let user = format!("璇锋€荤粨浠ヤ笅鏂囨湰锛歕n\n{text}");
    llm.complete(system, &user).await
}

/// Extracts readable text from a TypedValue (single doc, doc list, or raw).
fn extract_text(input: &TypedValue) -> String {
    if let Some(docs) = input.payload.as_array() {
        docs.iter()
            .filter_map(|d| d.get("content").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n\n")
    } else if let Some(content) = input.payload.get("content").and_then(Value::as_str) {
        content.to_string()
    } else if let Some(summary) = input.payload.get("summary").and_then(Value::as_str) {
        summary.to_string()
    } else {
        input.payload.to_string()
    }
}

/// Deterministic, dependency-free summarization fallback.
fn summarize_text(text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let char_count: usize = lines.iter().map(|l| l.len()).sum();
    let line_count = lines.len();

    let preview = lines
        .iter()
        .take(5)
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join(" | ");

    format!("Summary: {line_count} lines, {char_count} chars. Preview: {preview}")
}

/// Returns true if `cmd` is resolvable on PATH.
fn which(cmd: &str) -> bool {
    std::process::Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn summarize_counts_lines_and_chars() {
        let s = summarize_text("alpha\nbeta\ngamma\n");
        assert!(s.contains("3 lines"));
        assert!(s.contains("alpha"));
    }

    fn input_with_code(code: &str) -> TypedValue {
        TypedValue {
            value_type: ValueType::Scalar,
            payload: serde_json::json!({ "code": code }),
        }
    }

    #[tokio::test]
    async fn csv_inspect_schema_reports_columns_types_and_issues() {
        let dir = std::env::temp_dir().join("acos-csv-test");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("sales_q1.csv");
        let mut f = tokio::fs::File::create(&path).await.unwrap();
        f.write_all(b"date,product,units,revenue\na,1,5,100\nb,2,,\"1,200\"\n")
            .await
            .unwrap();

        let out = CsvInspectSchemaPrimitive
            .invoke(input_with_code(
                &serde_json::json!({ "path": path.display().to_string() }).to_string(),
            ))
            .await
            .unwrap();
        let stdout = out.payload.get("stdout").unwrap().as_str().unwrap();
        let report: Value = serde_json::from_str(stdout).unwrap();
        let cols = report["columns"].as_array().unwrap();
        assert_eq!(cols.len(), 4);
        assert_eq!(cols[3]["name"], "revenue");
        assert_eq!(cols[1]["type"], "integer");
        assert_eq!(cols[3]["type"], "number");
        assert!(report["issues"].as_array().unwrap().contains(&Value::String("missing_value".into())));
        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn csv_aggregate_sums_columns_and_rejects_unknown_columns() {
        let dir = std::env::temp_dir().join("acos-csv-agg");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join("sales_q2.csv");
        let mut f = tokio::fs::File::create(&path).await.unwrap();
        f.write_all(b"product,units,revenue\nx,10,\"$1,200\"\ny,5,300\n")
            .await
            .unwrap();

        let out = CsvAggregatePrimitive
            .invoke(input_with_code(
                &serde_json::json!({
                    "path": path.display().to_string(),
                    "columns": ["revenue", "units"]
                })
                .to_string(),
            ))
            .await
            .unwrap();
        let stdout = out.payload.get("stdout").unwrap().as_str().unwrap();
        let result: Value = serde_json::from_str(stdout).unwrap();
        let cols = result["columns"].as_array().unwrap();
        assert_eq!(cols[0]["name"], "revenue");
        assert_eq!(cols[0]["sum"], 1500.0);
        assert_eq!(cols[1]["sum"], 15.0);

        // Runtime schema enforcement: unknown column -> PrimitiveFailure.
        let err = CsvAggregatePrimitive
            .invoke(input_with_code(
                &serde_json::json!({
                    "path": path.display().to_string(),
                    "columns": ["quantity"]
                })
                .to_string(),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown column"), "got: {err}");
        tokio::fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn execute_python_injects_structured_inputs_when_enabled() {
        if !["python3", "python", "py"].iter().any(|c| which(c)) {
            return;
        }
        // Env var is process-global; set before and restore after.
        let prev = std::env::var("ACOS_STRUCTURED_INPUTS").ok();
        std::env::set_var("ACOS_STRUCTURED_INPUTS", "1");
        let out = ExecutePythonPrimitive
            .invoke(TypedValue {
                value_type: ValueType::Scalar,
                payload: serde_json::json!({
                    "code": "print(inputs['greeting'] + ' ' + inputs['name'])",
                    "greeting": "hello",
                    "name": "world"
                }),
            })
            .await
            .expect("python runs");
        match prev {
            Some(v) => std::env::set_var("ACOS_STRUCTURED_INPUTS", v),
            None => std::env::remove_var("ACOS_STRUCTURED_INPUTS"),
        }
        let stdout = out.payload.get("stdout").unwrap().as_str().unwrap();
        assert_eq!(stdout.trim(), "hello world");
    }
}
