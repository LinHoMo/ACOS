# P1-5B Formal Evaluation v0.1 harness (Windows PowerShell 5.1)
# Design: experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.1-design.md
# Freeze: main @ eb0d9a8 — do NOT modify Phase 1 code during runs.

param(
    [string]$Task = "tests/benchmarks/p1/flagship_csv_quality/acos_task.yaml",
    [string]$GT   = "tests/benchmarks/p1/flagship_csv_quality/expected/ground_truth.yaml",
    [string]$OutDir = "experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.1-results",
    [int]$ModelRuns = 5,
    [int]$RuleRuns = 5,
    [int]$BaselineRuns = 5,
    [string]$Goal = "Analyze all quarterly sales CSV files in the dataset directory. For each file: detect data-quality issues (schema drift, type errors, missing values, duplicates, outliers); repair recoverable issues; revalidate repaired inputs; compute quarterly statistics; merge results; run a quality review; and generate a consolidated Markdown report with an evidence log."
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$modelDir = Join-Path $OutDir "model"
$ruleDir  = Join-Path $OutDir "rule"
$baseDir  = Join-Path $OutDir "baseline"
New-Item -ItemType Directory -Force -Path $modelDir, $ruleDir, $baseDir | Out-Null

$summary = [ordered]@{}

# ---------- 1. ModelCompiler x N (probe: compile -> contract -> execute -> verify) ----------
Write-Host "=== P1-5B Formal v0.1: ModelCompiler x $ModelRuns ==="
& cargo run --quiet -p acos-cli --bin p1-5b-probe -- --runs $ModelRuns --task $Task --gt $GT --out-dir $modelDir
if ($LASTEXITCODE -ne 0) { Write-Error "p1-5b-probe failed" }

# ---------- 2. RuleCompiler x N (deterministic; compile -> execute -> verify) ----------
Write-Host "=== P1-5B Formal v0.1: RuleCompiler x $RuleRuns ==="
for ($i = 1; $i -le $RuleRuns; $i++) {
    $runId = "run-{0:000}" -f $i
    $out = & cargo run --quiet -p acos-cli -- run $Task --rules 2>&1 | Out-String
    $completed = $out -match "Run .*: Completed"
    $verdict = if ($out -match "Verification: (PASSED|FAILED)") { $Matches[1] } else { "UNKNOWN" }
    [ordered]@{ compile = if ($completed) { "pass" } else { "fail" }; execute = if ($completed) { "pass" } else { "fail" }; adequacy = $verdict.ToLower(); raw = $out } |
        ConvertTo-Json -Depth 3 | Set-Content (Join-Path $ruleDir "$runId.json")
    Write-Host ("  {0}: execute={1} adequacy={2}" -f $runId, $(if ($completed){"pass"}else{"fail"}), $verdict)
}

# ---------- 3. Baseline x N (direct tool loop -> verify) ----------
Write-Host "=== P1-5B Formal v0.1: Baseline x $BaselineRuns ==="
for ($i = 1; $i -le $BaselineRuns; $i++) {
    $runId = "run-{0:000}" -f $i
    $reportPath = Join-Path $baseDir "$runId-report.md"
    $out = & cargo run --quiet -p acos-cli -- baseline $Goal --verify $GT --output $reportPath 2>&1 | Out-String
    $verdict = if ($out -match "Overall: (PASSED|FAILED)") { $Matches[1] } else { "UNKNOWN" }
    $turns = if ($out -match "turns[=: ]+(\d+)") { $Matches[1] } else { "?" }
    [ordered]@{ execute = "pass"; adequacy = $verdict.ToLower(); turns = $turns; raw = $out } |
        ConvertTo-Json -Depth 3 | Set-Content (Join-Path $baseDir "$runId.json")
    Write-Host ("  {0}: adequacy={1} turns={2}" -f $runId, $verdict, $turns)
}

# ---------- 4. Aggregate summary ----------
Write-Host "=== Aggregating ==="
function Rate($obj) {
    [ordered]@{ passed = $obj.passed; total = $obj.total; rate = if ($obj.total) { "{0:P0}" -f ($obj.passed / $obj.total) } else { "N/A" } }
}
function Count-Flat($dir, $layer) {
    $passed = 0; $total = 0
    foreach ($f in (Get-ChildItem $dir -Filter "*.json")) {
        $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
        $total++
        if ($j.$layer -eq "pass" -or $j.$layer -eq "passed" -or $j.$layer -eq $true) { $passed++ }
    }
    Rate ([ordered]@{ passed = $passed; total = $total })
}
function Count-Model($layer) {
    $passed = 0; $total = 0
    foreach ($f in (Get-ChildItem $modelDir -Filter "*.trace.json")) {
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
    Rate ([ordered]@{ passed = $passed; total = $total })
}

$ruleC  = Count-Flat $ruleDir "compile"
$ruleA  = Count-Flat $ruleDir "adequacy"
$modelC = Count-Model "compile"
$modelK = Count-Model "contract"
$modelE = Count-Model "execute"
$modelA = Count-Model "adequacy"
$baseA  = Count-Flat $baseDir "adequacy"

$lines = @(
    "| 系统 | Compile | Contract | Execute | Adequacy |",
    "|---|---:|---:|---:|---:|",
    ("| RuleCompiler | {0} | N/A | {0} | {1} |" -f $ruleC.rate, $ruleA.rate),
    ("| ModelCompiler | {0} | {1} | {2} | {3} |" -f $modelC.rate, $modelK.rate, $modelE.rate, $modelA.rate),
    ("| Direct Tool Loop | N/A | N/A | N/A | {0} |" -f $baseA.rate)
)
$lines | Set-Content (Join-Path $OutDir "summary.md")
Write-Host ($lines -join "`n")
Write-Host ("Summary saved: {0}" -f (Join-Path $OutDir "summary.md"))