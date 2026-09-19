//! Explicit lifecycle transitions; networking is owned by the future runtime.
use serde::{Deserialize, Serialize};
use std::fmt;

/// Observable lifecycle state. A failed/stopped instance cannot restart itself.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum NodeState {
    /// Constructed, no resources acquired.
    #[default]
    Created,
    /// Resources and listeners are being initialized.
    Starting,
    /// Runtime is ready.
    Running,
    /// No new work accepted; outstanding operations are draining.
    Draining,
    /// Graceful shutdown completed.
    Stopped,
    /// Unrecoverable runtime failure.
    Failed,
}

/// Invalid lifecycle transition; the original state is retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionError {
    /// State before the attempted transition.
    pub from: NodeState,
    /// Requested state.
    pub to: NodeState,
}
impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid lifecycle transition {:?} -> {:?}",
            self.from, self.to
        )
    }
}
impl std::error::Error for TransitionError {}

/// Enforces lifecycle transitions; state cannot be modified directly.
#[derive(Debug, Default)]
pub struct Lifecycle {
    state: NodeState,
}
impl Lifecycle {
    /// Current state.
    pub fn state(&self) -> NodeState {
        self.state
    }
    /// Apply an allowed transition, leaving the current state unchanged on error.
    pub fn transition(&mut self, to: NodeState) -> Result<(), TransitionError> {
        use NodeState::*;
        if !matches!(
            (self.state, to),
            (Created, Starting)
                | (Starting, Running)
                | (Starting, Failed)
                | (Running, Draining)
                | (Running, Failed)
                | (Draining, Stopped)
                | (Draining, Failed)
        ) {
            return Err(TransitionError {
                from: self.state,
                to,
            });
        }
        self.state = to;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhaustive_transition_matrix() {
        use NodeState::*;
        let allowed = [
            (Created, Starting),
            (Starting, Running),
            (Starting, Failed),
            (Running, Draining),
            (Running, Failed),
            (Draining, Stopped),
            (Draining, Failed),
        ];
        for from in [Created, Starting, Running, Draining, Stopped, Failed] {
            for to in [Created, Starting, Running, Draining, Stopped, Failed] {
                let mut lifecycle = Lifecycle { state: from };
                let result = lifecycle.transition(to);
                assert_eq!(result.is_ok(), allowed.contains(&(from, to)));
                assert_eq!(lifecycle.state(), if result.is_ok() { to } else { from });
            }
        }
    }
    #[test]
    fn graceful_lifecycle_and_serialization() {
        let mut state = Lifecycle::default();
        for next in [
            NodeState::Starting,
            NodeState::Running,
            NodeState::Draining,
            NodeState::Stopped,
        ] {
            state.transition(next).unwrap();
        }
        assert_eq!(
            serde_json::to_string(&state.state()).unwrap(),
            "\"Stopped\""
        );
    }
}
