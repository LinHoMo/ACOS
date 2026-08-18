# P1-5B Behavioral Requirements

These are the behavioral requirements checked against generated CIR programs.
They measure **what the program DOES**, not its structure.

## Requirements

### BR-1: Multi-File Processing
The program processes ALL declared input files, not just one.
- **Check**: CIR references all TaskSpec.inputs paths (or uses a loop over them)
- **Pass criterion**: binding_accuracy == 1.0

### BR-2: Data Quality Analysis
The program includes nodes that detect data-quality issues.
- **Check**: CIR contains nodes for validation/detection (execute_python with validation logic, or multiple analysis stages)
- **Pass criterion**: at least one validation/analysis node beyond simple read

### BR-3: Anomaly Detection / Repair
The program attempts to repair recoverable issues.
- **Check**: CIR contains conditional logic or repair nodes
- **Pass criterion**: conditional node or explicit repair step

### BR-4: Structured Report
The program produces a structured output report.
- **Check**: CIR contains write_file with the declared output path/format
- **Pass criterion**: write_file node present with content from processing (not just pass-through)

### BR-5: Evidence / Audit Trail
The program produces evidence of its analysis.
- **Check**: CIR includes logging, evidence collection, or intermediate outputs
- **Pass criterion**: multiple output nodes or explicit evidence step

### BR-6: Control Flow Complexity
The program uses control structures appropriate to the task.
- **Check**: CIR contains at least one of: loop_map, conditional, retry
- **Pass criterion**: at least one control structure present

### BR-7: No Hallucinated Resources
The program only references declared inputs and capability-produced outputs.
- **Check**: All `path` values in read_file inputs match declared inputs or are capability outputs
- **Pass criterion**: no /tmp/... or other undeclared paths

## Scoring

- **Per requirement**: PASS / FAIL / PARTIAL
- **Overall**: count of PASS / 7
