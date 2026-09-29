use serde::{Deserialize, Serialize};

/// Origin of a line captured from a child process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// Incremental update emitted by a long-running operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// A plan step began.
    StepStarted {
        /// Stable step identifier.
        step: String,
        /// Human-readable step description.
        message: String,
    },
    /// A plan step completed.
    StepFinished {
        /// Stable step identifier.
        step: String,
        /// Human-readable completion description.
        message: String,
    },
    /// Reversal of a completed step began.
    CompensationStarted {
        /// Stable identifier of the step being reversed.
        step: String,
        /// Human-readable compensation description.
        message: String,
    },
    /// Reversal of a completed step finished.
    CompensationFinished {
        /// Stable identifier of the step being reversed.
        step: String,
        /// Whether compensation succeeded.
        succeeded: bool,
        /// Human-readable outcome.
        message: String,
    },
    /// Overall operation progress changed.
    Progress {
        /// Fraction from zero through one, or `None` when indeterminate.
        ratio: Option<f64>,
    },
    /// A child process emitted a line.
    Log {
        /// Stream that emitted the line.
        stream: LogStream,
        /// Line content without outlet-specific formatting.
        line: String,
    },
    /// The operation encountered a non-fatal condition.
    Warning {
        /// Human-readable warning.
        message: String,
    },
}
