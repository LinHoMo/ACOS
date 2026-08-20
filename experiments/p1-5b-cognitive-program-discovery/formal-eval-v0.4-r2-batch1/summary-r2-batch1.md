# P1-5B v0.4-R2 — Batch 1 (archived, INVALID / INFRA-CONFOUNDED)

> **Status**: INVALID / INFRA-CONFOUNDED — NOT used for H-E inference.
> **Date**: 2026-08-20 · **Frozen commit**: `3e9ef41` (spec: `docs/specs/2026-08-20-p1-5b-v0.4-r2-structured-execution-binding-controlled-design.md` FROZEN)
> **Design**: single-variable Control vs Treatment (Control = `--serialization-teaching` ×10; Treatment = `--serialization-teaching --structured-inputs` ×10)
> **Fixed conditions**: model=`deepseek-v4-flash`, provider=`openai`, base=`https://token.sensenova.cn/v1`, temperature=`0.0`, max_tokens=`32768`, timeout=`600s`, task/GT frozen, harness frozen, verifier frozen.

---

## 1. Outcome

| arm | runs | countable (non-external) | external | empty_response |
|---|---:|---:|---:|---:|
| Control | 10 | 1 | 9 | 0 |
| Treatment | 10 | 0 | 10 | 0 |

Failure-class matrix:

| class | Control | Treatment |
|---|---:|---:|
| execution_program | 1 | 0 |
| external (LLM API 429) | 9 | 10 |

Layer matrix (countable runs only): Control 1/1 compile PASS, 1/1 contract PASS, 0/1 execute; Treatment n=0.

**H-E verdict: INCONCLUSIVE — Treatment n=0 due to provider quota exhaustion; H-E not testable in Batch 1.** (Not a pass, not a fail.)

---

## 2. INFRA-001 — credential source precedence

- **Observation**: every preflight and run failed `401 Unauthorized {"error":{"code":16,"message":"Forbidden"}}` before the fix.
- **Root cause**: a session-level environment variable `LONGCAT_API_KEY` (legacy LongCat key, `ak_` prefix, len 32) took precedence over the `.env` SenseNova key (`sk-` prefix, len 35). The probe's `try_load_env` only fills UNSET vars (session > `.env` by design), so the wrong credential was used. The gateway rejects unknown credentials with `401 code 16`.
- **Verification that the key itself was valid**: same key via curl / .NET HttpClient / Python urllib → HTTP 200. reqwest/TLS-fingerprint hypotheses ruled out (the identical 401 body is produced for an explicitly invalid key, and the transport worked in v0.4).
- **Resolution**: zero code changes. Launch-time `Remove-Item Env:LONGCAT_API_KEY` so the probe loads the key from `.env`.
- **Recorded per run**: `metadata.security.key_present=true`, `raw_key_persisted=false`. No raw credential ever written to traces/logs/git.

## 3. INFRA-002 — provider quota regime (TPM exhaustion)

- **Observation**: run-001 (Control) consumed 32,847 tokens (reasoning model at max_tokens=32768); every subsequent run failed instantly with `429 Too Many Requests {"error":{"message":"inference tpm exhausted","type":"invalid_request_error","code":"429001"}}`.
- **Recorded per run**: `metadata.limits.observed_events=[429]` (run-001 additionally saw a transient `424` that recovered).
- **Conclusion**: the current provider quota regime cannot sustain 20 back-to-back 32k-token runs. This is a fixed-condition failure (Provider/Quota regime), the same class that confounded v0.4 S3 (5/5 empty responses) and v0.4 S1 (4/5 quarantined, LLM API 402).
- **No workaround applied**: `max_tokens`, model, prompt, task, and harness are unchanged (frozen). Any change would alter the experiment, not repeat it.

## 4. Retained Control evidence (n=1, non-statistical)

The single countable Control run (run-001) reproduces the v0.4 S1 signature:

- compile PASS (12 nodes), contract PASS, execute FAIL — generated Python died with `SyntaxError: unexpected character after line continuation character` (escaped-quote defect; real program bug)
- `env_refs=8/8` (100% env persistence), `inputs_refs=0` (0% binding adoption)
- repairs=3, total_tokens=32,847, wall=226,971 ms

Consistent with: serialization teaching ≠ structured execution binding. Kept as directional control evidence only (n=1, no statistical claim).

## 5. Quota demand estimate (for Batch 2 readiness)

- Expected runs: 20 (10+10). Worst-case tokens per run ≈ 32,847 (observed). Peak demand per chunk of 5 ≈ 160k+ inference tokens.
- The `429001` window recovered within minutes of idle in observation; capacity for consecutive 32k calls is NOT yet verified.
- Batch 2 start gates: credential_source = expected provider (session key removed), quota preflight = PASS, clock sanity = verified, chunked schedule (Control 5 → check → Control 5 → Treatment 5 → check → Treatment 5) with `observed_events` review between chunks.

---

*This batch is archived as-is (immutable). Traces, summary, and metadata are the formal record of a failed-attempt batch and are retained for history.*