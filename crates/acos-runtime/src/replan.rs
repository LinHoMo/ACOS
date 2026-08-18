//! Deterministic rule-based failure recovery.
//!
//! Rules inspect a [`FailureContext`] and, when they match, propose a
//! [`RecoveryProposal`] that the runtime validates and commits transactionally.

use acos_core::traits::Replanner;
use acos_core::types::{CirNodeKind, CirProgram, FailureClass, FailureContext, RecoveryProposal};

use serde_json::Value;

/// A recovery rule (capability-specific or capability-agnostic).
///
/// Rules are tried in registration order; the first rule whose
/// [`RecoveryRule::matches`] returns `true` and whose `propose` returns
/// `Some` wins.
pub trait RecoveryRule: Send + Sync + std::fmt::Debug {
    /// Whether this rule applies to the failure.
    fn matches(&self, failure: &FailureContext) -> bool;

    /// Proposes a recovery patch, or `None` if it cannot produce one.
    fn propose(
        &self,
        failure: &FailureContext,
        program: &CirProgram,
    ) -> Option<RecoveryProposal>;
}

/// Deterministic replanner: tries registered rules in order.
#[derive(Debug, Default)]
pub struct RuleReplanner {
    rules: Vec<Box<dyn RecoveryRule>>,
}

impl RuleReplanner {
    /// Creates an empty replanner (no rules).
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a recovery rule.
    pub fn with_rule(mut self, rule: Box<dyn RecoveryRule>) -> Self {
        self.rules.push(rule);
        self
    }
}

impl Replanner for RuleReplanner {
    fn propose(
        &self,
        failure: &FailureContext,
        program: &CirProgram,
    ) -> Option<RecoveryProposal> {
        for rule in &self.rules {
            if rule.matches(failure) {
                if let Some(proposal) = rule.propose(failure, program) {
                    return Some(proposal);
                }
            }
        }
        None
    }
}

/// Falls back to reading a local file when a node fails with a transient
/// class (timeout / rate limit / transient network error).
///
/// The failing node is replaced in place: the subgraph root keeps the failing
/// node's id and becomes a `read_file` invocation.
#[derive(Debug)]
pub struct OfflineFallbackRule {
    /// Path of the local fallback file.
    pub fallback_path: String,
}

impl RecoveryRule for OfflineFallbackRule {
    fn matches(&self, failure: &FailureContext) -> bool {
        matches!(
            failure.error_class,
            FailureClass::Timeout | FailureClass::RateLimit | FailureClass::TransientNetworkError
        )
    }

    fn propose(
        &self,
        failure: &FailureContext,
        program: &CirProgram,
    ) -> Option<RecoveryProposal> {
        let failing = program.nodes.iter().find(|n| n.node_id == failure.node_id)?;
        let mut root = failing.clone();
        root.kind = CirNodeKind::PrimitiveInvocation;
        root.capability = Some("read_file".into());
        root.children = vec![];
        root.control = None;
        root.inputs = vec![(
            "path".to_string(),
            Value::String(self.fallback_path.clone()),
        )]
        .into_iter()
        .collect();
        Some(RecoveryProposal {
            replace_node: failure.node_id.clone(),
            subgraph: vec![root],
            reason: format!(
                "{}: falling back to local file '{}'",
                failure.error_message, self.fallback_path
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acos_core::id::RunId;
    use acos_core::types::{CirNode, CirProgram, FailureClass, OutputSpec};
    use std::collections::HashMap;

    fn program_with_search() -> CirProgram {
        CirProgram {
            id: acos_core::id::ProgramId::new(),
            task_id: acos_core::id::TaskId(uuid::Uuid::new_v4()),
            entry: vec!["root".into()],
            nodes: vec![CirNode {
                kind: CirNodeKind::PrimitiveInvocation,
                node_id: "root".into(),
                capability: Some("search".into()),
                output: Some(OutputSpec {
                    name: "results".into(),
                    type_name: "String".into(),
                    fields: vec![],
                }),
                children: vec![],
                else_children: vec![],
                inputs: Default::default(),
                input_types: HashMap::new(),
                control: None,
            }],
            effects: vec![],
        }
    }

    fn failure(node_id: &str, class: FailureClass) -> FailureContext {
        FailureContext {
            run_id: RunId::new(),
            node_id: node_id.into(),
            error_class: class,
            error_message: "boom".into(),
            attempts: 0,
            recent_events: vec![],
        }
    }

    #[test]
    fn offline_fallback_rule_matches_transient_classes() {
        let rule = OfflineFallbackRule {
            fallback_path: "fallback.txt".into(),
        };
        assert!(rule.matches(&failure("root", FailureClass::Timeout)));
        assert!(rule.matches(&failure("root", FailureClass::TransientNetworkError)));
        assert!(!rule.matches(&failure("root", FailureClass::Unknown)));
        assert!(!rule.matches(&failure("root", FailureClass::InvalidInput)));
    }

    #[test]
    fn offline_fallback_rule_proposes_read_file_replacement() {
        let rule = OfflineFallbackRule {
            fallback_path: "fallback.txt".into(),
        };
        let program = program_with_search();
        let proposal = rule
            .propose(&failure("root", FailureClass::Timeout), &program)
            .unwrap();
        assert_eq!(proposal.replace_node, "root");
        let root = proposal.subgraph.first().unwrap();
        assert_eq!(root.node_id, "root");
        assert_eq!(root.capability.as_deref(), Some("read_file"));
        assert_eq!(
            root.inputs["path"],
            Value::String("fallback.txt".into())
        );
        assert_eq!(root.output.as_ref().map(|o| o.name.as_str()), Some("results"));
    }

    #[test]
    fn rule_replanner_returns_none_when_no_rule_matches() {
        let replanner = RuleReplanner::new()
            .with_rule(Box::new(OfflineFallbackRule {
                fallback_path: "x".into(),
            }));
        let program = program_with_search();
        let proposal = replanner.propose(&failure("root", FailureClass::Unknown), &program);
        assert!(proposal.is_none());
    }

    #[test]
    fn rule_replanner_returns_none_when_node_missing() {
        let replanner = RuleReplanner::new()
            .with_rule(Box::new(OfflineFallbackRule {
                fallback_path: "x".into(),
            }));
        let program = program_with_search();
        let proposal = replanner.propose(&failure("missing", FailureClass::Timeout), &program);
        assert!(proposal.is_none());
    }
}
