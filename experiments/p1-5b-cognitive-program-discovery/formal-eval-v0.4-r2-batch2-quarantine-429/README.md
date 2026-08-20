# P1-5B v0.4-R2 — Batch 2 quota-regime quarantine (429)

**Date**: 2026-08-20 · **Commit**: `25ab3c3` · **Frozen code commit**: `3e9ef41` (unchanged)

## What happened

Batch 2 Chunk 1 (Control runs 1–5, frozen harness flags, frozen env conditions,
`ACOS_EXP_COMMIT=25ab3c3`, `ACOS_EXP_CLOCK_SANITY=verified-offset--125s`) was
launched at 14:15 UTC. Result: **5/5 external** — all runs failed on the first
real-size call:

| run | error |
|---|---|
| run-001 | 429 `inference tpm exhausted` (429001) |
| run-002 | 429 `inference tpm exhausted` (429001) |
| run-003 | 429 `inference tpm exhausted` (429001) |
| run-004 | 429 `inference tpm exhausted` (429001) |
| run-005 | 429 `rpm exhausted` (quota_exceeded_error code 8) ← new error class |

## Regime signature

- `provider preflight: ok` (minimal completion passes)
- First real-size call in each run → immediate 429 (~100–300 ms)
- Same signature observed at 12:25 (Batch 1: 1 success then 19×429) and 14:17 (5/5)
- Conclusion: the current provider quota regime has effectively **zero
  inference budget for experiment-size calls** while tiny calls still pass.
  Batch 2 must NOT start under this regime (frozen gate: quota preflight must
  reflect real run demand; tiny-call preflight is insufficient — recorded here
  as a preflight lesson).

## Clock sanity (new finding)

w32tm NTP is blocked (UDP 123 timeout, `0x800705B4`); HTTPS Date-header and
gateway `created` timestamp cross-checks agree the host clock runs ~125 s slow.
Recorded as `verified-offset--125s` in run metadata.

## Batch 2 launch gate (updated)

Re-probe with a single experiment-size call (not a tiny completion) before
launching; only start when it returns 200 with meaningful completion tokens.