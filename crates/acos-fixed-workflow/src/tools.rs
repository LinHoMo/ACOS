//! Cross-platform Python interpreter detection and script execution.
//!
//! Mirrors the approach used by `acos-baseline` (P1-3) so that all four
//! experimental systems share the same tool-execution substrate.

/// Locate a usable Python interpreter on this platform.
#[cfg(windows)]
pub fn find_python() -> Option<&'static str> {
    ["python", "python3", "py"]
        .iter()
        .find(|cmd| {
            std::process::Command::new("where")
                .arg(cmd)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
        .copied()
}

/// Locate a usable Python interpreter on this platform.
#[cfg(not(windows))]
pub fn find_python() -> Option<&'static str> {
    ["python3", "python"]
        .iter()
        .find(|cmd| {
            std::process::Command::new("which")
                .arg(cmd)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
        .copied()
}

/// Run the fixed workflow script and capture its stdout (JSON summary).
pub async fn run_script(
    script_path: &str,
    args: &[&str],
) -> Result<(String, u64), String> {
    let python = find_python().ok_or_else(|| {
        "no python interpreter found on PATH (fixed workflow requires python)".to_string()
    })?;
    let start = std::time::Instant::now();
    let output = tokio::process::Command::new(python)
        .arg(script_path)
        .args(args)
        .output()
        .await
        .map_err(|e| format!("failed to spawn python: {e}"))?;
    let elapsed_ms = start.elapsed().as_millis() as u64;
    if !output.status.success() {
        return Err(format!(
            "script exited {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok((String::from_utf8_lossy(&output.stdout).into_owned(), elapsed_ms))
}