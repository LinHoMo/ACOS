//! Fixture-as-contract loading.
//!
//! Fixtures are YAML files under `<fixtures_dir>/<suite>/<id>.yaml`. Each one
//! declares a [`Fixture`] with an embedded [`CirProgram`] (or a compiler goal)
//! plus the [`Expected`] outcome.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use acos_core::types::CirProgram;
use serde::Deserialize;

/// How a fixture's program is produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FixtureMode {
    /// Run the inline `cir` (validated, then executed). Compile column = PASS.
    #[default]
    Cir,
    /// Run the compiler pipeline (`compiler` decides rules vs model).
    Run,
}

/// Which compiler backend to use; only meaningful when mode = run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompilerKind {
    /// Rule-based compiler.
    Rules,
    /// Model-based compiler.
    Model,
}

/// A single benchmark fixture (a behavioral contract).
#[derive(Debug, Clone, Deserialize)]
pub struct Fixture {
    /// Unique fixture id (within its suite).
    pub id: String,
    /// Compiler mode: inline cir (default) vs full pipeline.
    #[serde(default)]
    pub mode: FixtureMode,
    /// Which compiler backend to use; only meaningful when mode = run.
    pub compiler: Option<CompilerKind>,
    /// Natural-language goal (documentation only).
    pub goal: String,
    /// Inline CIR program, when mode = cir.
    pub cir: Option<CirProgram>,
    /// Files written into the scratch workspace; `{workspace}` is substituted
    /// into `inputs` before execution.
    #[serde(default)]
    pub files: HashMap<String, String>,
    /// Expected outcome.
    pub expected: Expected,
}

/// Expected outcome of a fixture run.
#[derive(Debug, Clone, Deserialize)]
pub struct Expected {
    /// Pass when compile errors (validation) are expected.
    #[serde(default)]
    pub compile: Option<bool>,
    /// Pass when the run is expected to complete successfully.
    #[serde(default)]
    pub execution: Option<bool>,
    /// Pass when verification (acos-verify) is expected to pass.
    #[serde(default)]
    pub verification: Option<bool>,
    /// Expected recovery label, e.g. `retry`, `rule`, `model`.
    pub recovery: Option<String>,
    /// Expected final status string of the run, e.g. `success`, `failed`.
    pub final_status: Option<String>,
    /// Expected validation rejection reason substring (negative fixtures).
    pub validation: Option<String>,
}

/// Loads `fixtures_dir/**/*.yaml`, grouped by top-level directory (suite).
pub fn load_fixtures(fixtures_dir: &Path) -> Vec<(String, Fixture)> {
    let mut out = Vec::new();
    let mut stack = vec![fixtures_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "yaml") {
                let suite = path
                    .parent()
                    .and_then(Path::file_name)
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "misc".into());
                let text = std::fs::read_to_string(&path).expect("read fixture");
                let fixture: Fixture = serde_yaml::from_str(&text).expect("parse fixture");
                out.push((suite, fixture));
            }
        }
    }
    out.sort_by(|a, b| a.1.id.cmp(&b.1.id));
    out
}

/// Creates a scratch workspace under a temp dir, writes `files`, returns the
/// workspace path. The runtime must be given a `FileStore` rooted there.
pub fn prepare_workspace(fixture: &Fixture) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("acos-bench-{}", uuid::Uuid::new_v4()));
    for (name, content) in &fixture.files {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(path, content).expect("write fixture file");
    }
    dir
}

/// Substitutes `{workspace}` tokens in a string input.
pub fn substitute_workspace(input: &str, workspace: &str) -> String {
    input.replace("{workspace}", workspace)
}

impl Fixture {
    /// Returns the fixture files with `{workspace}` already substituted.
    pub fn files_in(&self, workspace: &str) -> HashMap<String, String> {
        self.files
            .iter()
            .map(|(k, v)| (k.clone(), substitute_workspace(v, workspace)))
            .collect()
    }
}
