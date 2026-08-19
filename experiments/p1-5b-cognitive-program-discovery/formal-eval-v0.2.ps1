# P1-5B Formal Evaluation v0.2 harness (Windows PowerShell 5.1)
# Spec: docs/specs/2026-08-19-modelcompiler-v0.2-structured-program-synthesis-design.md (FROZEN)
# Freeze: main @ 90c8917 — do NOT modify pipeline code during runs.
#
# Experiment A (Control Flow Discovery) + Experiment B (Two-stage Compilation)
# + Experiment C (Data Contract Integration) are all driven by the same
# `p1-5b-probe --plan` runs; B compares against frozen v0.1 data, C reads
# contract/repair/binding stats from the traces.
#
# Requires LONGCAT_API_KEY in the environment or .env.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.2.ps1

param(
    [string]$Task = "tests/benchmarks/p1/flagship_csv_quality/acos_task.yaml",
    [string]$GT   = "tests/benchmarks/p1/flagship_csv_quality/expected/ground_truth.yaml",
    [string]$OutDir = "experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.2-results",
    [int]$ModelRuns = 5,
    [switch]$AggregateOnly
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

# Ensure a python interpreter is reachable for execute_python primitives
# (probe uses `which` on python3/python/py; Windows needs the install dir on PATH).
$pyCandidates = @(
    "C:\Users\Lin\AppData\Local\Programs\Python\Python312",
    "$env:LOCALAPPDATA\Programs\Python\Python312",
    "$env:LOCALAPPDATA\Programs\Python\Python313"
)
foreach ($p in $pyCandidates) {
    if (Test-Path (Join-Path $p "python.exe")) {
        $env:Path = "$p;$env:Path"
        Write-Host "python found: $p"
        break
    }
}
if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
    Write-Warning "no python on PATH — execute_python runs will fail; install Python 3.12+"
}

# ---------- Experiment A + B + C: ModelCompiler (Plan IR) x N ----------
if (-not $AggregateOnly) {
    Write-Host "=== P1-5B Formal v0.2: ModelCompiler (Plan IR) x $ModelRuns ==="
    & cargo run --quiet -p acos-cli --bin p1-5b-probe -- --runs $ModelRuns --task $Task --gt $GT --out-dir $OutDir --plan
    if ($LASTEXITCODE -ne 0) { Write-Error "p1-5b-probe --plan failed" }
} else {
    Write-Host "=== Aggregate only (existing traces in $OutDir) ==="
}

# ---------- Aggregation ----------
$traces = Get-ChildItem $OutDir -Filter "*.trace.json" | Sort-Object Name

function Get-Count($dir, $layer) {
    $passed = 0; $total = 0
    foreach ($f in (Get-ChildItem $dir -Filter "*.trace.json")) {
        $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
        $total++
        $ok = switch ($layer) {
            "compile"  { $j.run.compile_success -eq $true }
            "contract" { $j.contract.pass -eq $true }
            "execute"  { $j.execution.ok -eq $true }
            "adequacy" { $j.execution.verification.ok -eq $true -and $j.execution.verification.passed -eq $true }
        }
        if ($ok) { $passed++ }
    }
    [ordered]@{ passed = $passed; total = $total; rate = if ($total) { "{0:P0}" -f ($passed / $total) } else { "N/A" } }
}

function Get-Avg($dir, $expr) {
    $vals = @()
    foreach ($f in (Get-ChildItem $dir -Filter "*.trace.json")) {
        $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
        $v = & $expr $j
        if ($null -ne $v) { $vals += [double]$v }
    }
    if ($vals.Count) { "{0:N2}" -f (($vals | Measure-Object -Average).Average) } else { "N/A" }
}

# Per-run plan metrics (row table)
$rows = @()
$intentTot = 0; $adoptedTot = 0; $compTot = 0; $compSum = 0.0; $coverTot = 0; $coverSum = 0.0
foreach ($f in $traces) {
    $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
    $pm = $j.plan_metrics
    $gm = $j.program_metrics
    $intent = if ($pm) { [int]$pm.control_intent_count } else { 0 }
    $adopted = if ($gm -and $j.run.compile_success) {
        [int]$gm.loop_count + [int]$gm.condition_count + [int]$gm.retry_count
    } else { 0 }
    # Plan completeness: 6 behavioral requirements of P1-FLAGSHIP-001
    $completeness = 0
    if ($pm) {
        if ([int]$pm.foreach_count -ge 1) { $completeness++ }                       # iterate all inputs
        if ([int]$pm.conditional_count -ge 1) { $completeness++ }                   # conditional repair
        if ([int]$pm.retry_count -ge 1) { $completeness++ }                         # retry on transient failure
        if ($j.output.final_plan.steps | Where-Object { $_.capability -eq "write_file" }) { $completeness++ }   # report artifact
        if ([int]$pm.step_count -ge 5) { $completeness++ }                          # multi-stage pipeline
        if ($j.output.final_plan.dataFlow.Count -ge 2) { $completeness++ }          # data-flow wiring
    }
    # Control coverage: required foreach/conditional/retry (3) vs generated
    $cover = if ($pm) { [math]::Min(([int]$pm.foreach_count + [int]$pm.conditional_count + [int]$pm.retry_count), 3) / 3.0 } else { 0 }
    $recall = if ($intent -gt 0) { $adopted / $intent } else { 1.0 }
    $intentTot += $intent; $adoptedTot += $adopted
    $compTot++; $compSum += $completeness / 6.0; $coverTot++; $coverSum += $cover
    $rows += [ordered]@{
        run = $f.BaseName
        compile = if ($j.run.compile_success) { "pass" } else { "fail" }
        contract = if ($j.contract.pass) { "pass" } else { "fail" }
        execute = if ($j.execution.ok) { "pass" } else { "fail" }
        adequacy = if ($j.execution.verification.ok -and $j.execution.verification.passed) { "pass" } else { "fail" }
        repairs = if ($j.repair_tax) { [int]$j.repair_tax.repair_attempts_used } else { 0 }
        intent = $intent
        adopted = $adopted
        recall = "{0:P0}" -f $recall
        completeness = "{0:P0}" -f ($completeness / 6.0)
        coverage = "{0:P0}" -f $cover
    }
}

$modelC = Get-Count $OutDir "compile"
$modelK = Get-Count $OutDir "contract"
$modelE = Get-Count $OutDir "execute"
$modelA = Get-Count $OutDir "adequacy"

$recallAvg = if ($intentTot) { "{0:P0}" -f ($adoptedTot / $intentTot) } else { "N/A" }
$compAvg = if ($compTot) { "{0:P0}" -f ($compSum / $compTot) } else { "N/A" }
$coverAvg = if ($coverTot) { "{0:P0}" -f ($coverSum / $coverTot) } else { "N/A" }

# Contract integration (Experiment C): violations surfaced at compile time
$cViolations = 0; $cFailures = 0; $bindClosed = 0; $bindTotal = 0
foreach ($f in $traces) {
    $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
    $cFailures += [int]$j.repair_tax.repair_attempts_used
    if (-not $j.run.compile_success -and $j.run.final_error) { $cViolations++ }
    $plan = $j.output.final_plan
    if ($plan) {
        $declared = @()
        foreach ($s in $plan.steps) { if ($s.inputBindings) { $declared += $s.inputBindings } }
        $bindTotal += $declared.Count
        $names = @()
        foreach ($s in $plan.steps) { $names += $s.name }
        foreach ($b in $declared) {
            if ($names -contains $b.source) { $bindClosed++ }
        }
    }
}

# ---------- Proposition B ----------
$propositionB = ($modelC.passed / $modelC.total -ge 0.8) -and ($compSum / $compTot -ge 0.7) -and ($modelA.passed / $modelA.total -ge 0.6)
$verdictB = if ($propositionB) { "SUPPORTED (Compile >= 80% AND Plan completeness >= 70% AND Adequacy >= 60%)" } else { "NOT SUPPORTED" }

# ---------- Report ----------
$lines = @(
    "## P1-5B Formal Evaluation v0.2 (Structured Program Synthesis)",
    "",
    "- Runs: $ModelRuns x ModelCompiler (Plan IR), P1-FLAGSHIP-001, LongCat-2.0",
    "- Spec: docs/specs/2026-08-19-modelcompiler-v0.2-structured-program-synthesis-design.md (FROZEN)",
    "",
    "### Layer matrix (v0.2 vs frozen v0.1)",
    "",
    "| System | Compile | Contract | Execute | Adequacy |",
    "|---|---:|---:|---:|---:|",
    ("| ModelCompiler v0.2 (Plan IR) | {0} | {1} | {2} | {3} |" -f $modelC.rate, $modelK.rate, $modelE.rate, $modelA.rate),
    ("| ModelCompiler v0.1 (direct CIR, frozen) | see formal-eval-v0.1-results | | | |"),
    "",
    "### Per-run detail (Experiment A)",
    "",
    "| run | compile | contract | execute | adequacy | repairs | control intent | adopted | recall | completeness | coverage |",
    "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|"
)
foreach ($r in $rows) {
    $lines += ("| {0} | {1} | {2} | {3} | {4} | {5} | {6} | {7} | {8} | {9} | {10} |" -f $r.run, $r.compile, $r.contract, $r.execute, $r.adequacy, $r.repairs, $r.intent, $r.adopted, $r.recall, $r.completeness, $r.coverage)
}
$lines += @(
    "",
    "### Plan metrics (Experiment A)",
    "",
    ("- Control Intent Recall: {0} (adopted {1} / {2} declared control intents)" -f $recallAvg, $adoptedTot, $intentTot),
    ("- Plan completeness (avg): {0} (6 behavioral requirements)" -f $compAvg),
    ("- Control coverage (avg): {0} (required foreach/conditional/retry = 3)" -f $coverAvg),
    "",
    "### Two-stage comparison (Experiment B, vs frozen v0.1)",
    "",
    ("- v0.1 Compile 1/5 (20%), v0.2 Compile {0}/{1} ({2})" -f $modelC.passed, $modelC.total, $modelC.rate),
    ("- Repair Tax: v0.2 average repairs per run = {0} (first-pass success {1}/{2})" -f (Get-Avg $OutDir { param($j) if ($j.repair_tax) { $j.repair_tax.repair_attempts_used } }), ($traces | Where-Object { (Get-Content $_.FullName -Raw | ConvertFrom-Json).repair_tax.first_pass_success }).Count, $traces.Count),
    "",
    "### Contract integration (Experiment C)",
    "",
    ("- Compile-time contract failures surfaced (final_error): {0}" -f $cViolations),
    ("- Repair attempts (contract violations caught by repair loop): {0}" -f $cFailures),
    ("- Plan binding closure: {0}/{1} ({2:P0})" -f $bindClosed, $bindTotal, $(if ($bindTotal) { $bindClosed / $bindTotal } else { 0 })),
    "",
    "## Proposition B verdict",
    "",
    ("- **{0}**" -f $verdictB),
    ""
)
$lines | Set-Content (Join-Path $OutDir "summary.md")
Write-Host ($lines -join "`n")
Write-Host ("Summary saved: {0}" -f (Join-Path $OutDir "summary.md"))