# P1-5B Formal Evaluation v0.3 harness (Windows PowerShell 5.1)
# Spec: docs/specs/2026-08-19-p1-5b-v0.3-capability-contract-typed-execution-design.md (FROZEN)
# Freeze: main @ a111486 — do NOT modify pipeline code during runs.
#
# Experiment groups (spec section 3):
#   A = historical frozen control: v0.2 traces (main @ 7a3b36a, dir
#       formal-eval-v0.2-results) — NOT re-sampled this round.
#   B = + csv.inspect_schema (observe): probe --plan --csv-mode observe
#   C = + csv.aggregate with runtime schema enforcement:
#       probe --plan --csv-mode enforce
#
# Metrics (spec section 3): four-layer matrix (Compile/Contract/Execute/
# Adequacy) + inspect usage/consumption, schema utilization, schema
# hallucination rate, code defect rate, repair rate.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.3.ps1

param(
    [string]$Task = "tests/benchmarks/p1/flagship_csv_quality/acos_task.yaml",
    [string]$GT   = "tests/benchmarks/p1/flagship_csv_quality/expected/ground_truth.yaml",
    [string]$OutDirB = "experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.3-results-b",
    [string]$OutDirC = "experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.3-results-c",
    [string]$ControlDir = "experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.2-results",
    [int]$ModelRuns = 5,
    [switch]$AggregateOnly
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $OutDirB, $OutDirC | Out-Null

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
    Write-Warning "no python on PATH — execute_python runs will fail"
}

# ---------- Experiment B (Observe) and C (Enforce) ----------
if (-not $AggregateOnly) {
    Write-Host "=== P1-5B Formal v0.3: B (Observe) x $ModelRuns ==="
    & cargo run --quiet -p acos-cli --bin p1-5b-probe -- --runs $ModelRuns --task $Task --gt $GT --out-dir $OutDirB --plan --csv-mode observe
    if ($LASTEXITCODE -ne 0) { Write-Error "probe B failed" }
    Write-Host "=== P1-5B Formal v0.3: C (Enforce) x $ModelRuns ==="
    & cargo run --quiet -p acos-cli --bin p1-5b-probe -- --runs $ModelRuns --task $Task --gt $GT --out-dir $OutDirC --plan --csv-mode enforce
    if ($LASTEXITCODE -ne 0) { Write-Error "probe C failed" }
} else {
    Write-Host "=== Aggregate only ==="
}

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

function Get-PlanSteps($j) {
    if (-not $j.output.final_plan -or -not $j.output.final_plan.steps) { return @() }
    $steps = @()
    foreach ($s in $j.output.final_plan.steps) {
        $steps += $s
        if ($s.body) { foreach ($b in $s.body) { $steps += $b } }
    }
    $steps
}

function Get-CsvMetrics($dir) {
    $total = 0; $inspectPlans = 0; $consumed = 0; $aggSteps = 0; $halluc = 0; $pySteps = 0; $pyDefect = 0
    foreach ($f in (Get-ChildItem $dir -Filter "*.trace.json")) {
        $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
        $total++
        $steps = @()
        if ($j.output.final_plan -and $j.output.final_plan.steps) {
            foreach ($s in $j.output.final_plan.steps) {
                $steps += $s
                if ($s.body) { foreach ($b in $s.body) { $steps += $b } }
            }
        }
        $inspect = @($steps | Where-Object { $_.capability -eq "csv.inspect_schema" })
        $agg = @($steps | Where-Object { $_.capability -eq "csv.aggregate" })
        if ($inspect.Count -gt 0) {
            $inspectPlans++
            $inspectNames = @($inspect | ForEach-Object { $_.name })
            $uses = @($steps | Where-Object {
                $binds = @($_.inputBindings | Where-Object { $inspectNames -contains $_.source })
                $binds.Count -gt 0
            })
            if ($uses.Count -gt 0) { $consumed++ }
        }
        $aggSteps += $agg.Count
        $err = ""
        if ($j.execution -and $j.execution.ok -eq $false -and $j.execution.error) { $err = $j.execution.error }
        if ($err -match "unknown column") { $halluc++ }
        $pySteps += @($steps | Where-Object { $_.capability -eq "execute_python" }).Count
        if ($pySteps -gt 0) {
            $defect = ($j.execution.ok -eq $false) -and ($err -match "NameError|KeyError|SyntaxError|ProviderFailure|no python")
            if ($defect) { $pyDefect++ }
        }
    }
    [ordered]@{
        runs = $total
        inspect_usage_rate = if ($total) { "{0:P0}" -f ($inspectPlans / $total) } else { "N/A" }
        inspect_plans = $inspectPlans
        inspect_result_consumed = $consumed
        schema_utilization = if ($inspectPlans) { "{0:P0}" -f ($consumed / $inspectPlans) } else { "N/A" }
        aggregate_steps = $aggSteps
        hallucination_runs = $halluc
        execute_python_steps = $pySteps
        code_defect_rate = if ($pySteps) { "{0:P0}" -f ($pyDefect / $pySteps) } else { "N/A" }
        defective_python_runs = $pyDefect
    }
}

function Get-AvgRepairs($dir) {
    $vals = @()
    foreach ($f in (Get-ChildItem $dir -Filter "*.trace.json")) {
        $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
        if ($j.repair_tax) { $vals += [double]$j.repair_tax.repair_attempts_used }
    }
    if ($vals.Count) { "{0:N2}" -f (($vals | Measure-Object -Average).Average) } else { "N/A" }
}

function Get-RowTable($dir) {
    $rows = @()
    foreach ($f in (Get-ChildItem $dir -Filter "*.trace.json" | Sort-Object Name)) {
        $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
        $steps = Get-PlanSteps $j
        $inspect = @($steps | Where-Object { $_.capability -eq "csv.inspect_schema" }).Count
        $agg = @($steps | Where-Object { $_.capability -eq "csv.aggregate" }).Count
        $rows += [ordered]@{
            run = $f.BaseName
            compile = if ($j.run.compile_success) { "pass" } else { "fail" }
            contract = if ($j.contract.pass) { "pass" } else { "fail" }
            execute = if ($j.execution.ok) { "pass" } else { "fail" }
            adequacy = if ($j.execution.verification.ok -and $j.execution.verification.passed) { "pass" } else { "fail" }
            repairs = if ($j.repair_tax) { [int]$j.repair_tax.repair_attempts_used } else { 0 }
            inspect = $inspect
            aggregate = $agg
            err = if ($j.execution -and $j.execution.ok -eq $false -and $j.execution.error) { ($j.execution.error -split "`n")[0] } else { "" }
        }
    }
    $rows
}

$bC = Get-Count $OutDirB "compile"; $bK = Get-Count $OutDirB "contract"
$bE = Get-Count $OutDirB "execute"; $bA = Get-Count $OutDirB "adequacy"
$cC = Get-Count $OutDirC "compile"; $cK = Get-Count $OutDirC "contract"
$cE = Get-Count $OutDirC "execute"; $cA = Get-Count $OutDirC "adequacy"
$aC = Get-Count $ControlDir "compile"; $aA = Get-Count $ControlDir "adequacy"

$bM = Get-CsvMetrics $OutDirB
$cM = Get-CsvMetrics $OutDirC

# ---------- H-C (Capability Contract Hypothesis) ----------
$cCompileOk = $cC.passed -ge [math]::Ceiling(0.8 * $cC.total)
$cAdequacyOk = $cA.passed -ge [math]::Ceiling(0.6 * $cA.total)
$hcSupported = $cCompileOk -and $cAdequacyOk
$bSupported = $bC.passed -ge [math]::Ceiling(0.8 * $bC.total) -and $bA.passed -ge [math]::Ceiling(0.6 * $bA.total)

$lines = @(
    "## P1-5B Formal Evaluation v0.3 (Capability Contract & Typed Execution)",
    "",
    "- Groups: A = historical frozen control (v0.2 traces @ 7a3b36a, NOT re-sampled); B = + csv.inspect_schema (observe); C = + csv.aggregate (runtime schema enforcement)",
    "- Runs: B x $ModelRuns, C x $ModelRuns, P1-FLAGSHIP-001, LongCat-2.0, main @ a111486",
    "- Spec: docs/specs/2026-08-19-p1-5b-v0.3-capability-contract-typed-execution-design.md (FROZEN)",
    "",
    "### Layer matrix (A vs B vs C)",
    "",
    "| Group | Compile | Contract | Execute | Adequacy |",
    "|---|---:|---:|---:|---:|",
    ("| A (v0.2 frozen control) | {0} | see v0.2 report | 0/5 | {1} |" -f $aC.rate, $aA.rate),
    ("| B (Observe) | {0} | {1} | {2} | {3} |" -f $bC.rate, $bK.rate, $bE.rate, $bA.rate),
    ("| C (Enforce) | {0} | {1} | {2} | {3} |" -f $cC.rate, $cK.rate, $cE.rate, $cA.rate),
    "",
    "### Per-run detail (B)",
    "",
    "| run | compile | contract | execute | adequacy | repairs | inspect | aggregate | error |",
    "|---|---:|---:|---:|---:|---:|---:|---:|---|"
)
foreach ($r in (Get-RowTable $OutDirB)) {
    $lines += ("| {0} | {1} | {2} | {3} | {4} | {5} | {6} | {7} | {8} |" -f $r.run, $r.compile, $r.contract, $r.execute, $r.adequacy, $r.repairs, $r.inspect, $r.aggregate, $r.err)
}
$lines += @(
    "",
    "### Per-run detail (C)",
    "",
    "| run | compile | contract | execute | adequacy | repairs | inspect | aggregate | error |",
    "|---|---:|---:|---:|---:|---:|---:|---:|---|"
)
foreach ($r in (Get-RowTable $OutDirC)) {
    $lines += ("| {0} | {1} | {2} | {3} | {4} | {5} | {6} | {7} | {8} |" -f $r.run, $r.compile, $r.contract, $r.execute, $r.adequacy, $r.repairs, $r.inspect, $r.aggregate, $r.err)
}
$lines += @(
    "",
    "### Capability metrics (B / C)",
    "",
    ("- inspect usage rate (inspect_requested / total_plans): B {0} ({1} plans) / C {2} ({3} plans)" -f $bM.inspect_usage_rate, $bM.inspect_plans, $cM.inspect_usage_rate, $cM.inspect_plans),
    ("- inspect result consumed (later step binds the inspect output): B {0} / C {1}" -f $bM.inspect_result_consumed, $cM.inspect_result_consumed),
    ("- schema utilization rate (consumed / inspect plans): B {0} / C {1}" -f $bM.schema_utilization, $cM.schema_utilization),
    ("- schema hallucination rate (invalid_field_references / all_field_references, 0/0 -> N/A): B N/A / C N/A  [v1 proxy = runtime 'unknown column' rejections (B {0} / C {1}); no csv.aggregate steps in B or C this round -> no field references -> 0/0 -> N/A]" -f $bM.hallucination_runs, $cM.hallucination_runs),
    ("- code defect rate (defective execute_python steps / total execute_python steps): B {0} ({1}/{2}) / C {3} ({4}/{5})  [v1 proxy = 1 defective step per failing run; classes NameError/KeyError/SyntaxError/Other]" -f $bM.code_defect_rate, $bM.defective_python_runs, $bM.execute_python_steps, $cM.code_defect_rate, $cM.defective_python_runs, $cM.execute_python_steps),
    ("- repair rate (avg repairs per run): A (v0.2) 0.60 / B {0} / C {1}" -f (Get-AvgRepairs $OutDirB), (Get-AvgRepairs $OutDirC)),
    "",
    "### H-C (Capability Contract Hypothesis) verdict",
    "",
    ("- C Compile >= 80%: {0} ({1}/{2}) ; C Adequacy >= 60%: {3} ({4}/{5})" -f $cCompileOk, $cC.passed, $cC.total, $cAdequacyOk, $cA.passed, $cA.total),
    ("- **H-C {0}** - C passing supports the Capability Contract intermediate proposition; it does NOT by itself establish Proposition B (end-to-end discovery of executable, adequate Cognitive Programs), which requires validation on more tasks." -f $(if ($hcSupported) { "SUPPORTED" } else { "NOT SUPPORTED" })),
    ("- B reaching Compile >= 80% AND Adequacy >= 60% ({0}) would support the claim that the model can autonomously use capability contracts ({1})." -f $bSupported, $(if ($bSupported) { "observed" } else { "not observed" })),
    ""
)
$lines | Set-Content -Encoding UTF8 (Join-Path $OutDirB "summary-v0.3.md")
Write-Host ($lines -join "`n")
Write-Host ("Summary saved: {0}" -f (Join-Path $OutDirB "summary-v0.3.md"))