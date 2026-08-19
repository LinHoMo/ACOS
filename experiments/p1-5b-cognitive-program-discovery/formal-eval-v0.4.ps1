# P1-5B Formal Evaluation v0.4 harness (Windows PowerShell 5.1)
# Spec: docs/specs/2026-08-19-p1-5b-v0.4-primitive-invocation-contract-structured-inputs-design.md (FROZEN)
# Freeze: main @ 719fc60 — do NOT modify pipeline code during runs.
#
# Experiment arms (2x2 factor design, spec section 7):
#   S0 = historical frozen control (v0.3 C traces, commit f613592) — NOT re-sampled
#   S1 = serialization teaching only          (--serialization-teaching)
#   S2 = Structured Inputs Package only       (--structured-inputs)
#   S3 = serialization teaching + Package     (both flags)
#
# Metrics (spec section 8): four-layer matrix + serialization_failure_rate /
# env_failure_rate / env_persistence_rate / repair_rate. Denominators of 0 -> N/A.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.4.ps1

param(
    [string]$Task = "tests/benchmarks/p1/flagship_csv_quality/acos_task.yaml",
    [string]$GT   = "tests/benchmarks/p1/flagship_csv_quality/expected/ground_truth.yaml",
    [string]$OutDirS1 = "experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.4-results-s1",
    [string]$OutDirS2 = "experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.4-results-s2",
    [string]$OutDirS3 = "experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.4-results-s3",
    [string]$ControlDir = "experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.3-results-c",
    [int]$ModelRuns = 5,
    [int[]]$Arm = @(1, 2, 3),
    [switch]$AggregateOnly
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $OutDirS1, $OutDirS2, $OutDirS3 | Out-Null

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

function Invoke-Arm($flagArgs, $dir, $label) {
    Write-Host "=== v0.4: $label x $ModelRuns -> $dir ==="
    & cargo run --quiet -p acos-cli --bin p1-5b-probe -- `
        --runs $ModelRuns --task $Task --gt $GT --out-dir $dir --plan --csv-mode enforce @flagArgs
    if ($LASTEXITCODE -ne 0) { Write-Error "probe $label failed" }
}

if (-not $AggregateOnly) {
    if ($Arm -contains 1) { Invoke-Arm @("--serialization-teaching") $OutDirS1 "S1 (serialization)" }
    if ($Arm -contains 2) { Invoke-Arm @("--structured-inputs") $OutDirS2 "S2 (structured inputs)" }
    if ($Arm -contains 3) { Invoke-Arm @("--serialization-teaching", "--structured-inputs") $OutDirS3 "S3 (combined)" }
} else {
    Write-Host "=== Aggregate only ==="
}

function Get-PlanSteps($j) {
    $steps = @()
    if ($j.output.final_plan -and $j.output.final_plan.steps) {
        foreach ($s in $j.output.final_plan.steps) {
            $steps += $s
            if ($s.body) { foreach ($b in $s.body) { $steps += $b } }
        }
    }
    $steps
}

function Get-Layer($dir, $layer) {
    $passed = 0; $total = 0
    foreach ($f in (Get-ChildItem $dir -Filter "run-*.trace.json" | Sort-Object Name)) {
        $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
        $total++
        $ok = switch ($layer) {
            "compile"  { $j.run.compile_success -eq $true }
            "contract" { $j.contract.pass -eq $true }
            "execute"  { $j.execution.ok -eq $true }
            "adequacy" { $j.execution.ok -eq $true -and $j.execution.verification.ok -eq $true -and $j.execution.verification.passed -eq $true }
        }
        if ($ok) { $passed++ }
    }
    [ordered]@{ passed = $passed; total = $total; rate = if ($total) { "{0:P0}" -f ($passed / $total) } else { "N/A" } }
}

function Get-V04Metrics($dir) {
    $total = 0; $pyRuns = 0; $serFail = 0; $envFail = 0; $envPersist = 0; $reps = @()
    foreach ($f in (Get-ChildItem $dir -Filter "run-*.trace.json" | Sort-Object Name)) {
        $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
        $total++
        $steps = Get-PlanSteps $j
        $hasPy = @($steps | Where-Object { $_.capability -eq "execute_python" }).Count -gt 0
        $envInCode = @($steps | Where-Object { $_.code -match "\benv\s*[\[.]" }).Count -gt 0
        $err = ""
        if ($j.execution -and $j.execution.ok -eq $false -and $j.execution.error) { $err = $j.execution.error }
        $ser = ($err -match "invalid type: map")
        if ($ser) { $serFail++ }
        $envClass = ($envInCode -and ($err -match "InvalidPythonBinding|NameError: name 'env'")) -or ($err -match "InvalidPythonBinding")
        if ($envClass) { $envFail++ }
        if ($hasPy) {
            $pyRuns++
            if ($envInCode) { $envPersist++ }
        }
        if ($j.repair_tax) { $reps += [int]$j.repair_tax.repair_attempts_used }
    }
    $repAvg = if ($reps.Count) { "{0:N2}" -f (($reps | Measure-Object -Average).Average) } else { "N/A" }
    [ordered]@{
        total = $total
        serialization_failure_rate = if ($total) { "{0:P0}" -f ($serFail / $total) } else { "N/A" }
        serialization_failures = $serFail
        py_runs = $pyRuns
        env_failure_rate = if ($pyRuns) { "{0:P0}" -f ($envFail / $pyRuns) } else { "N/A" }
        env_failures = $envFail
        env_persistence_rate = if ($pyRuns) { "{0:P0}" -f ($envPersist / $pyRuns) } else { "N/A" }
        env_persistent_runs = $envPersist
        repair_rate_avg = $repAvg
    }
}

function Get-RowTable($dir) {
    $rows = @()
    foreach ($f in (Get-ChildItem $dir -Filter "run-*.trace.json" | Sort-Object Name)) {
        $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
        $steps = Get-PlanSteps $j
        $err = if ($j.execution -and $j.execution.ok -eq $false -and $j.execution.error) { ($j.execution.error -split "`n")[0] } else { "" }
        $envInCode = @($steps | Where-Object { $_.code -match "\benv\s*[\[.]" }).Count
        $rows += [ordered]@{
            run = $f.BaseName
            compile = if ($j.run.compile_success) { "pass" } else { "fail" }
            contract = if ($j.contract.pass) { "pass" } else { "fail" }
            execute = if ($j.execution.ok) { "pass" } else { "fail" }
            adequacy = if ($j.execution.ok -and $j.execution.verification.ok -and $j.execution.verification.passed) { "pass" } else { "fail" }
            repairs = if ($j.repair_tax) { [int]$j.repair_tax.repair_attempts_used } else { 0 }
            env_code_refs = $envInCode
            err = $err
        }
    }
    $rows
}

function Get-ControlMetrics($dir) {
    # Historical v0.3 C: frozen baseline for S0.
    $total = 0; $pyRuns = 0; $serFail = 0; $envFail = 0; $envPersist = 0; $reps = @()
    foreach ($f in (Get-ChildItem $dir -Filter "run-*.trace.json" | Sort-Object Name)) {
        $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
        $total++
        $steps = Get-PlanSteps $j
        $hasPy = @($steps | Where-Object { $_.capability -eq "execute_python" }).Count -gt 0
        $envInCode = @($steps | Where-Object { $_.code -match "\benv\s*[\[.]" }).Count -gt 0
        $err = ""
        if ($j.execution -and $j.execution.ok -eq $false -and $j.execution.error) { $err = $j.execution.error }
        if ($err -match "invalid type: map") { $serFail++ }
        $envClass = $envInCode -and ($err -match "NameError: name 'env'")
        if ($envClass) { $envFail++ }
        if ($hasPy) { $pyRuns++; if ($envInCode) { $envPersist++ } }
        if ($j.repair_tax) { $reps += [int]$j.repair_tax.repair_attempts_used }
    }
    $repAvg = if ($reps.Count) { "{0:N2}" -f (($reps | Measure-Object -Average).Average) } else { "N/A" }
    [ordered]@{
        total = $total
        serialization_failure_rate = if ($total) { "{0:P0}" -f ($serFail / $total) } else { "N/A" }
        serialization_failures = $serFail
        py_runs = $pyRuns
        env_failure_rate = if ($pyRuns) { "{0:P0}" -f ($envFail / $pyRuns) } else { "N/A" }
        env_failures = $envFail
        env_persistence_rate = if ($pyRuns) { "{0:P0}" -f ($envPersist / $pyRuns) } else { "N/A" }
        env_persistent_runs = $envPersist
        repair_rate_avg = $repAvg
    }
}

$s0 = Get-ControlMetrics $ControlDir
$s1c = Get-Layer $OutDirS1 "compile"; $s1k = Get-Layer $OutDirS1 "contract"; $s1e = Get-Layer $OutDirS1 "execute"; $s1a = Get-Layer $OutDirS1 "adequacy"
$s2c = Get-Layer $OutDirS2 "compile"; $s2k = Get-Layer $OutDirS2 "contract"; $s2e = Get-Layer $OutDirS2 "execute"; $s2a = Get-Layer $OutDirS2 "adequacy"
$s3c = Get-Layer $OutDirS3 "compile"; $s3k = Get-Layer $OutDirS3 "contract"; $s3e = Get-Layer $OutDirS3 "execute"; $s3a = Get-Layer $OutDirS3 "adequacy"
$m1 = Get-V04Metrics $OutDirS1
$m2 = Get-V04Metrics $OutDirS2
$m3 = Get-V04Metrics $OutDirS3

# Success criteria (spec section 9).
$hS = $m1.serialization_failure_rate -match "^0%" -or $m1.serialization_failures -le 1   # <= 0.2 with n=5 => <= 1
$hE = ($m2.env_failures -le 1) -and ($m2.env_persistent_runs -le 1)                       # <= 0.2 with n=5 => <= 1
$s3CompileOk = $s3c.passed -ge 4
$s3Best = $s3e.passed -ge [Math]::Max($s1e.passed, $s2e.passed) -and $s3a.passed -ge [Math]::Max($s1a.passed, $s2a.passed)

$lines = @(
    "## P1-5B Formal Evaluation v0.4 (Primitive Invocation Contract & Structured Inputs)",
    "",
    "- Groups: S0 = historical frozen control (v0.3 C traces @ f613592, NOT re-sampled); S1 = serialization teaching; S2 = Structured Inputs Package; S3 = combined",
    "- Runs: S1/S2/S3 x $ModelRuns each (fresh samples), P1-FLAGSHIP-001, LongCat-2.0, main @ 719fc60",
    "- Spec: docs/specs/2026-08-19-p1-5b-v0.4-primitive-invocation-contract-structured-inputs-design.md (FROZEN)",
    "",
    "### Layer matrix (S0 vs S1/S2/S3)",
    "",
    "| Group | Compile | Contract | Execute | Adequacy |",
    "|---|---:|---:|---:|---:|",
    "| S0 (v0.3 C frozen) | 20% | 20% | 0% | 0% |",
    ("| S1 (+serialization) | {0} | {1} | {2} | {3} |" -f $s1c.rate, $s1k.rate, $s1e.rate, $s1a.rate),
    ("| S2 (+structured inputs) | {0} | {1} | {2} | {3} |" -f $s2c.rate, $s2k.rate, $s2e.rate, $s2a.rate),
    ("| S3 (combined) | {0} | {1} | {2} | {3} |" -f $s3c.rate, $s3k.rate, $s3e.rate, $s3a.rate),
    "",
    "### Per-run detail (S1)",
    "",
    "| run | compile | contract | execute | adequacy | repairs | env_code_refs | error |",
    "|---|---:|---:|---:|---:|---:|---:|---|"
)
foreach ($r in (Get-RowTable $OutDirS1)) {
    $lines += ("| {0} | {1} | {2} | {3} | {4} | {5} | {6} | {7} |" -f $r.run, $r.compile, $r.contract, $r.execute, $r.adequacy, $r.repairs, $r.env_code_refs, $r.err)
}
$lines += @("", "### Per-run detail (S2)", "", "| run | compile | contract | execute | adequacy | repairs | env_code_refs | error |", "|---|---:|---:|---:|---:|---:|---:|---|")
foreach ($r in (Get-RowTable $OutDirS2)) {
    $lines += ("| {0} | {1} | {2} | {3} | {4} | {5} | {6} | {7} |" -f $r.run, $r.compile, $r.contract, $r.execute, $r.adequacy, $r.repairs, $r.env_code_refs, $r.err)
}
$lines += @("", "### Per-run detail (S3)", "", "| run | compile | contract | execute | adequacy | repairs | env_code_refs | error |", "|---|---:|---:|---:|---:|---:|---:|---|")
foreach ($r in (Get-RowTable $OutDirS3)) {
    $lines += ("| {0} | {1} | {2} | {3} | {4} | {5} | {6} | {7} |" -f $r.run, $r.compile, $r.contract, $r.execute, $r.adequacy, $r.repairs, $r.env_code_refs, $r.err)
}
$lines += @(
    "",
    "### Primitive Invocation metrics (S0 / S1 / S2 / S3)",
    "",
    ("- serialization_failure_rate (code-as-map compile failures / total runs): S0 {0} ({1}/5) / S1 {2} ({3}/5) / S2 {4} ({5}/5) / S3 {6} ({7}/5)" -f $s0.serialization_failure_rate, $s0.serialization_failures, $m1.serialization_failure_rate, $m1.serialization_failures, $m2.serialization_failure_rate, $m2.serialization_failures, $m3.serialization_failure_rate, $m3.serialization_failures),
    ("- env_failure_rate (env-class failing runs / runs with execute_python in final plan): S0 {0} ({1}/{2}) / S1 {3} ({4}/{5}) / S2 {6} ({7}/{8}) / S3 {9} ({10}/{11})" -f $s0.env_failure_rate, $s0.env_failures, $s0.py_runs, $m1.env_failure_rate, $m1.env_failures, $m1.py_runs, $m2.env_failure_rate, $m2.env_failures, $m2.py_runs, $m3.env_failure_rate, $m3.env_failures, $m3.py_runs),
    ("- env_persistence_rate (final plan code still references env / py runs): S0 {0} ({1}/{2}) / S1 {3} ({4}/{5}) / S2 {6} ({7}/{8}) / S3 {9} ({10}/{11})" -f $s0.env_persistence_rate, $s0.env_persistent_runs, $s0.py_runs, $m1.env_persistence_rate, $m1.env_persistent_runs, $m1.py_runs, $m2.env_persistence_rate, $m2.env_persistent_runs, $m2.py_runs, $m3.env_persistence_rate, $m3.env_persistent_runs, $m3.py_runs),
    ("- repair_rate (avg repairs per run): S0 {0} / S1 {1} / S2 {2} / S3 {3}" -f $s0.repair_rate_avg, $m1.repair_rate_avg, $m2.repair_rate_avg, $m3.repair_rate_avg),
    "",
    "### Verdicts (spec section 9)",
    "",
    ("- **H-S (Serialization Contract)**: SUPPORTED if serialization_failure_rate(S1) <= 0.2 (<= 1/5). S1 = {0} ({1}/5) -> **{2}**" -f $m1.serialization_failure_rate, $m1.serialization_failures, $(if ($hS) { "SUPPORTED" } else { "NOT SUPPORTED" })),
    ("- **H-E (Structured Execution Binding Contract)**: SUPPORTED if env_failure_rate(S2) <= 0.2 AND env_persistence_rate(S2) <= 0.2. S2 = {0} ({1}) / {2} ({3}) -> **{4}**" -f $m2.env_failure_rate, $m2.env_failures, $m2.env_persistence_rate, $m2.env_persistent_runs, $(if ($hE) { "SUPPORTED" } else { "NOT SUPPORTED" })),
    ("- **S3 interaction**: SUPPORTED if Compile(S3) >= 80% AND Execute/Adequacy(S3) >= max(S1,S2). S3 = {0} / {1} -> **{2}**" -f $s3c.rate, $s3e.rate, $(if ($s3CompileOk -and $s3Best) { "INTERACTION" } else { "NO INTERACTION" })),
    "",
    "- H-S/H-E are intermediate propositions; Proposition B is out of scope for v0.4 (multi-task validation required).",
    ""
)
$lines | Set-Content -Encoding UTF8 (Join-Path $OutDirS1 "summary-v0.4.md")
Write-Host ($lines -join "`n")
Write-Host ("Summary saved: {0}" -f (Join-Path $OutDirS1 "summary-v0.4.md"))