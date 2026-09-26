use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    StepStarted { step: String, message: String },
    StepFinished { step: String, message: String },
    Progress { ratio: Option<f64> },
    Log { stream: LogStream, line: String },
    Warning { message: String },
}
