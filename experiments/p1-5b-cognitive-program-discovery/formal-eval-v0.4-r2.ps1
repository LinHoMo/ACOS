# P1-5B Formal Evaluation v0.4-R2 harness (Windows PowerShell 5.1)
# Spec: docs/specs/2026-08-20-p1-5b-v0.4-r2-structured-execution-binding-controlled-design.md
#
# Single-variable design (H-E controlled evaluation):
#   Control   = --serialization-teaching                    (S1 config: old binding, env tolerated)
#   Treatment = --serialization-teaching --structured-inputs (S3 config: inputs injection + env rejection)
#
# Fixed conditions (spec section 2), all recorded automatically in run metadata:
#   model / provider / temperature / max_tokens / timeout / commit / clock sanity
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.4-r2.ps1

param(
    [string]$Task = "tests/benchmarks/p1/flagship_csv_quality/acos_task.yaml",
    [string]$GT   = "tests/benchmarks/p1/flagship_csv_quality/expected/ground_truth.yaml",
    [string]$ControlDir = "experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.4-r2-results-control",
    [string]$TreatmentDir = "experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.4-r2-results-treatment",
    [int]$ModelRuns = 10,
    [string]$Model = "deepseek-v4-flash",
    [string]$Provider = "openai",
    [string]$Temperature = "0.0",
    [string]$MaxTokens = "32768",
    [string]$TimeoutSeconds = "600",
    [switch]$ControlOnly,
    [switch]$TreatmentOnly,
    [switch]$AggregateOnly
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Force -Path $ControlDir, $TreatmentDir | Out-Null

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

# ── Fixed experiment conditions (harness-injected, auto-recorded) ──────────────
$expCommit = (& git rev-parse HEAD 2>$null | Out-String).Trim()
if (-not $expCommit) { $expCommit = "unknown" }
$env:ACOS_EXP_COMMIT = $expCommit
$env:ACOS_LLM_MODEL = $Model
$env:ACOS_LLM_PROVIDER = $Provider
if ($Temperature) { $env:ACOS_LLM_TEMPERATURE = $Temperature } else { Remove-Item Env:ACOS_LLM_TEMPERATURE -ErrorAction SilentlyContinue }
$env:ACOS_LLM_MAX_TOKENS = $MaxTokens
$env:ACOS_LLM_TIMEOUT_SECONDS = $TimeoutSeconds

# ── Clock sanity pre-flight (spec §2; fallback: unverified) ────────────────────
$clockSanity = "unverified"
try {
    $w32 = & w32tm /stripchart /computer:time.windows.com /samples:1 2>&1 | Out-String
    if ($LASTEXITCODE -eq 0 -and $w32 -match "offset:\s*([-+]?\d+(\.\d+)?)s") {
        $clockSanity = "verified-offset-$($Matches[1])s"
    } elseif ($w32 -match "Successful") {
        $clockSanity = "verified-offset-unknown"
    }
} catch { $clockSanity = "unverified" }
$env:ACOS_EXP_CLOCK_SANITY = $clockSanity
Write-Host "clock sanity: $clockSanity"
Write-Host "experiment commit: $expCommit"

$sessionStart = (Get-Date).ToUniversalTime().ToString("o")

function Invoke-Arm($flagArgs, $dir, $label) {
    Write-Host "=== v0.4-R2: $label x $ModelRuns -> $dir ==="
    & cargo run --quiet -p acos-cli --bin p1-5b-probe -- `
        --runs $ModelRuns --task $Task --gt $GT --out-dir $dir --plan --csv-mode enforce @flagArgs
    if ($LASTEXITCODE -ne 0) { Write-Error "probe $label failed" }
}

if (-not $AggregateOnly) {
    if (-not $TreatmentOnly) { Invoke-Arm @("--serialization-teaching") $ControlDir "Control (old binding)" }
    if (-not $ControlOnly)   { Invoke-Arm @("--serialization-teaching", "--structured-inputs") $TreatmentDir "Treatment (structured inputs)" }
} else {
    Write-Host "=== Aggregate only ==="
}

# ── Aggregation ────────────────────────────────────────────────────────────────

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

function Is-ExternalFailure($j) {
    $j.run.final_error -and $j.run.final_error -match "external system failure"
}

function Is-EmptyResponse($j) {
    $j.output -and $j.output.initial_raw_response -eq ""
}

function Get-RunClass($j) {
    if (Is-ExternalFailure $j) { return "external" }
    if (Is-EmptyResponse $j) { return "empty_response" }
    $err = ""
    if ($j.execution -and $j.execution.ok -eq $false -and $j.execution.error) { $err = $j.execution.error }
    $finalErr = if ($j.run.final_error) { $j.run.final_error } else { "" }
    if (-not $j.run.compile_success) {
        if ($finalErr -match "invalid type: map") { return "compile_serialization" }
        if ($finalErr -match "InvalidPythonBinding") { return "compile_env_rejected" }
        return "compile_other"
    }
    if (-not $j.contract.pass) { return "contract" }
    if ($j.execution -and $j.execution.ok -eq $false) {
        if ($err -match "NameError: name 'env'") { return "execution_env" }
        return "execution_program"
    }
    if ($j.execution -and $j.execution.ok) { return "execution_ok" }
    return "unknown"
}

function Get-ArmStats($dir) {
    $total = 0; $external = 0; $empty = 0
    $pyRuns = 0; $envFail = 0; $envPersist = 0; $serFail = 0
    $adoptRuns = 0; $pySteps = 0; $adoptSteps = 0
    $reps = @(); $walls = @(); $tokens = @()
    $incompleteMeta = @()
    $classCount = @{}
    $rows = @()

    foreach ($f in (Get-ChildItem $dir -Filter "run-*.trace.json" | Sort-Object Name)) {
        $j = Get-Content $f.FullName -Raw | ConvertFrom-Json
        $class = Get-RunClass $j
        $classCount[$class] = [int]$classCount[$class] + 1
        if ($class -eq "external") { $external++; continue }
        $total++
        if ($class -eq "empty_response") { $empty++ }

        if (-not $j.metadata -or -not $j.metadata.experiment -or $j.metadata.experiment.commit -eq "" -or $j.metadata.experiment.commit -eq "unknown") {
            $incompleteMeta += $f.Name
        }

        $steps = Get-PlanSteps $j
        $pyStepsList = @($steps | Where-Object { $_.capability -eq "execute_python" -and $_.code })
        $hasPy = $pyStepsList.Count -gt 0
        $envInCode = @($pyStepsList | Where-Object { $_.code -match "\benv\s*[\[.]" }).Count
        $inputsInCode = @($pyStepsList | Where-Object { $_.code -match "inputs\s*\[" }).Count
        $err = ""
        if ($j.execution -and $j.execution.ok -eq $false -and $j.execution.error) { $err = $j.execution.error }
        $finalErr = if ($j.run.final_error) { $j.run.final_error } else { "" }

        if ($err -match "invalid type: map" -or $finalErr -match "invalid type: map") { $serFail++ }
        $envClass = ($envInCode -gt 0 -and ($err -match "InvalidPythonBinding|NameError: name 'env'")) -or ($err -match "InvalidPythonBinding") -or ($finalErr -match "InvalidPythonBinding")
        if ($envClass) { $envFail++ }
        if ($hasPy) {
            $pyRuns++
            if ($envInCode -gt 0) { $envPersist++ }
            if ($inputsInCode -gt 0) { $adoptRuns++ }
            $pySteps += $pyStepsList.Count
            $adoptSteps += $inputsInCode
        }
        if ($j.repair_tax) { $reps += [int]$j.repair_tax.repair_attempts_used }
        if ($j.timing) { $walls += [long]$j.timing.total_wall_ms }
        if ($j.usage) { $tokens += [long]$j.usage.total_tokens }

        $rows += [ordered]@{
            run = $f.BaseName
            class = $class
            compile = if ($j.run.compile_success) { "pass" } else { "fail" }
            contract = if ($j.contract.pass) { "pass" } else { "fail" }
            execute = if ($j.execution.ok) { "pass" } else { "fail" }
            adequacy = if ($j.execution.ok -and $j.execution.verification.ok -and $j.execution.verification.passed) { "pass" } else { "fail" }
            repairs = if ($j.repair_tax) { [int]$j.repair_tax.repair_attempts_used } else { 0 }
            py_steps = $pyStepsList.Count
            env_refs = $envInCode
            inputs_refs = $inputsInCode
            tokens = if ($j.usage) { [long]$j.usage.total_tokens } else { 0 }
            wall_ms = if ($j.timing) { [long]$j.timing.total_wall_ms } else { 0 }
        }
    }

    $avg = { param($list) if ($list.Count) { "{0:N0}" -f (($list | Measure-Object -Average).Average) } else { "N/A" } }
    $med = { param($list) if ($list.Count) { $s = $list | Sort-Object; $n = $s.Count; if ($n % 2) { $s[[int]($n/2)] } else { [long](([long]$s[$n/2 - 1] + [long]$s[$n/2]) / 2) } } else { "N/A" } }

    [ordered]@{
        total = $total
        external = $external
        empty_response = $empty
        classes = $classCount
        compile_pass = @($rows | Where-Object { $_.compile -eq "pass" }).Count
        contract_pass = @($rows | Where-Object { $_.contract -eq "pass" }).Count
        execute_pass = @($rows | Where-Object { $_.execute -eq "pass" }).Count
        adequacy_pass = @($rows | Where-Object { $_.adequacy -eq "pass" }).Count
        py_runs = $pyRuns
        serialization_failures = $serFail
        serialization_failure_rate = if ($total - $empty) { "{0:P0}" -f ($serFail / ($total - $empty)) } else { "N/A" }
        env_failures = $envFail
        env_failure_rate = if ($pyRuns) { "{0:P0}" -f ($envFail / $pyRuns) } else { "N/A" }
        env_persistent_runs = $envPersist
        env_persistence_rate = if ($pyRuns) { "{0:P0}" -f ($envPersist / $pyRuns) } else { "N/A" }
        binding_adoption_runs = $adoptRuns
        binding_adoption_rate = if ($pyRuns) { "{0:P0}" -f ($adoptRuns / $pyRuns) } else { "N/A" }
        binding_adoption_steps = if ($pySteps) { "{0:P0}" -f ($adoptSteps / $pySteps) } else { "N/A" }
        repair_rate_avg = if ($reps.Count) { "{0:N2}" -f (($reps | Measure-Object -Average).Average) } else { "N/A" }
        latency_wall_ms_avg = & $avg $walls
        latency_wall_ms_median = & $med $walls
        token_cost_avg = & $avg $tokens
        token_cost_median = & $med $tokens
        metadata_incomplete = $incompleteMeta
        rows = $rows
    }
}

$c = Get-ArmStats $ControlDir
$t = Get-ArmStats $TreatmentDir

# ── Verdict gates (spec section 5) ─────────────────────────────────────────────
$g1adopt = $t.binding_adoption_rate -match "^8[0-9]%|^9[0-9]%|^100%" -and $c.binding_adoption_rate -match "^0%|^1[0-9]%|^20%"
$g2env = $t.env_failure_rate -match "^0%|^1[0-9]%|^20%" -and $t.env_persistence_rate -match "^0%|^1[0-9]%|^20%" -and $c.env_failure_rate -match "^[5-9][0-9]%|^100%"
$g3exec = $t.execute_pass -ge ($c.execute_pass - 1)
$g4adeq = $t.adequacy_pass -ge $c.adequacy_pass
$g5empty = $t.empty_response -lt 5
$hE = $g1adopt -and $g2env -and $g3exec -and $g4adeq -and $g5empty
$verdict = if (-not $g5empty) { "NOT JUDGABLE (empty-response >= 50%)" } elseif ($hE) { "H-E SUPPORTED" } else { "H-E INCONCLUSIVE" }

$lines = @(
    "## P1-5B Formal Evaluation v0.4-R2 (Structured Execution Binding — H-E controlled)",
    "",
    "- Design: single-variable Control vs Treatment (spec docs/specs/2026-08-20-p1-5b-v0.4-r2-structured-execution-binding-controlled-design.md)",
    "- Runs: Control x $($c.total) / Treatment x $($t.total) (target $ModelRuns each); task = P1-FLAGSHIP-001",
    "- Fixed conditions: model=$Model provider=$Provider temperature=$Temperature max_tokens=$MaxTokens timeout=${TimeoutSeconds}s commit=$expCommit clock_sanity=$clockSanity",
    "- Session start (UTC): $sessionStart",
    "",
    "### Failure-class matrix",
    "",
    "| class | Control | Treatment |",
    "|---|---:|---:|"
)
foreach ($k in ($c.classes.Keys | Sort-Object)) {
    $lines += ("| {0} | {1} | {2} |" -f $k, $c.classes[$k], $t.classes[$k])
}
$lines += @(
    "",
    "### Layer matrix",
    "",
    "| arm | Compile | Contract | Execute | Adequacy |",
    "|---|---:|---:|---:|---:|",
    ("| Control | {0}/{1} | {2}/{1} | {3}/{1} | {4}/{1} |" -f $c.compile_pass, $c.total, $c.contract_pass, $c.execute_pass, $c.adequacy_pass),
    ("| Treatment | {0}/{1} | {2}/{1} | {3}/{1} | {4}/{1} |" -f $t.compile_pass, $t.total, $t.contract_pass, $t.execute_pass, $t.adequacy_pass),
    "",
    "### Key metrics",
    "",
    ("- serialization_failure_rate: Control {0} ({1}) / Treatment {2} ({3})" -f $c.serialization_failure_rate, $c.serialization_failures, $t.serialization_failure_rate, $t.serialization_failures),
    ("- env_failure_rate: Control {0} ({1}/{2}) / Treatment {3} ({4}/{5})" -f $c.env_failure_rate, $c.env_failures, $c.py_runs, $t.env_failure_rate, $t.env_failures, $t.py_runs),
    ("- env_persistence_rate: Control {0} ({1}/{2}) / Treatment {3} ({4}/{5})" -f $c.env_persistence_rate, $c.env_persistent_runs, $c.py_runs, $t.env_persistence_rate, $t.env_persistent_runs, $t.py_runs),
    ("- binding_adoption_rate (run-level): Control {0} ({1}/{2}) / Treatment {3} ({4}/{5})" -f $c.binding_adoption_rate, $c.binding_adoption_runs, $c.py_runs, $t.binding_adoption_rate, $t.binding_adoption_runs, $t.py_runs),
    ("- binding_adoption_rate (step-level): Control {0} / Treatment {1}" -f $c.binding_adoption_steps, $t.binding_adoption_steps),
    ("- empty_response_rate: Control {0}/{1} / Treatment {2}/{3}" -f $c.empty_response, $c.total, $t.empty_response, $t.total),
    ("- repair_rate: Control {0} / Treatment {1}" -f $c.repair_rate_avg, $t.repair_rate_avg),
    ("- latency wall_ms: Control avg {0} / med {1}; Treatment avg {2} / med {3}" -f $c.latency_wall_ms_avg, $c.latency_wall_ms_median, $t.latency_wall_ms_avg, $t.latency_wall_ms_median),
    ("- token cost (total_tokens/run): Control avg {0} / med {1}; Treatment avg {2} / med {3}" -f $c.token_cost_avg, $c.token_cost_median, $t.token_cost_avg, $t.token_cost_median),
    "",
    "### Per-run detail (Control)",
    "",
    "| run | class | compile | contract | execute | adequacy | repairs | py_steps | env_refs | inputs_refs | tokens | wall_ms |",
    "|---|---|---|---|---|---|---:|---:|---:|---:|---:|---:|"
)
foreach ($r in $c.rows) {
    $lines += ("| {0} | {1} | {2} | {3} | {4} | {5} | {6} | {7} | {8} | {9} | {10} | {11} |" -f $r.run, $r.class, $r.compile, $r.contract, $r.execute, $r.adequacy, $r.repairs, $r.py_steps, $r.env_refs, $r.inputs_refs, $r.tokens, $r.wall_ms)
}
$lines += @("", "### Per-run detail (Treatment)", "", "| run | class | compile | contract | execute | adequacy | repairs | py_steps | env_refs | inputs_refs | tokens | wall_ms |", "|---|---|---|---|---|---|---:|---:|---:|---:|---:|---:|")
foreach ($r in $t.rows) {
    $lines += ("| {0} | {1} | {2} | {3} | {4} | {5} | {6} | {7} | {8} | {9} | {10} | {11} |" -f $r.run, $r.class, $r.compile, $r.contract, $r.execute, $r.adequacy, $r.repairs, $r.py_steps, $r.env_refs, $r.inputs_refs, $r.tokens, $r.wall_ms)
}
$lines += @(
    "",
    "### Verdict gates (spec section 5)",
    "",
    ("- G1 binding_adoption: Treatment >= 0.8 AND Control <= 0.2 -> {0} ({1} / {2})" -f $(if ($g1adopt) { "PASS" } else { "FAIL" }), $t.binding_adoption_rate, $c.binding_adoption_rate),
    ("- G2 env: Treatment env_failure <= 0.2 AND env_persistence <= 0.2 AND Control env_failure >= 0.5 -> {0}" -f $(if ($g2env) { "PASS" } else { "FAIL" })),
    ("- G3 execute: Treatment >= Control - 1 -> {0} ({1} vs {2})" -f $(if ($g3exec) { "PASS" } else { "FAIL" }), $t.execute_pass, $c.execute_pass),
    ("- G4 adequacy: Treatment >= Control -> {0} ({1} vs {2})" -f $(if ($g4adeq) { "PASS" } else { "FAIL" }), $t.adequacy_pass, $c.adequacy_pass),
    ("- G5 empty response: Treatment < 5/10 -> {0} ({1})" -f $(if ($g5empty) { "PASS" } else { "FAIL" }), $t.empty_response),
    "",
    ("**H-E VERDICT: {0}**" -f $verdict),
    "",
    "- H-E SUPPORTED requires G1-G5 all PASS. H-E is an intermediate proposition; Proposition B is out of scope.",
    ""
)
if ($c.metadata_incomplete.Count -or $t.metadata_incomplete.Count) {
    $lines += @("### Metadata completeness", "",
        "- INCOMPLETE (excluded from causal conclusions): Control: $($c.metadata_incomplete -join ', ') ; Treatment: $($t.metadata_incomplete -join ', ')")
} else {
    $lines += @("### Metadata completeness", "", "- All runs carry complete metadata (commit/provider/model/temperature/max_tokens/timeout/clock/key).")
}
$lines += ""

$summaryPath = Join-Path $TreatmentDir "summary-r2.md"
$lines | Set-Content -Encoding UTF8 $summaryPath
Write-Host ($lines -join "`n")
Write-Host ("Summary saved: {0}" -f $summaryPath)

# ── Session metadata (experiment-metadata.json) ────────────────────────────────
$sessionMeta = [ordered]@{
    experiment = [ordered]@{
        id = "p1-5b-v0.4-r2"
        commit = $expCommit
        provider = $Provider
        model = $Model
        temperature = $Temperature
        max_tokens = $MaxTokens
        timeout_seconds = $TimeoutSeconds
    }
    environment = [ordered]@{
        os = "windows"
        timezone = $env:TZ
        session_start_utc = $sessionStart
        clock_sanity_check = $clockSanity
    }
    design = [ordered]@{
        control = "serialization_teaching_only (S1 config; old execution binding)"
        treatment = "serialization_teaching + structured_inputs (S3 config; inputs injection + env prohibition)"
        runs_per_arm = $ModelRuns
        task = $Task
        gt = $GT
        single_variable = "Structured Inputs Package"
    }
    verdict = $verdict
}
$sessionMeta | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 (Join-Path $TreatmentDir "experiment-metadata.json")
Write-Host ("Session metadata saved: {0}" -f (Join-Path $TreatmentDir "experiment-metadata.json"))