use avdkit_model::{
    Capability, CapabilityId, CapabilityMatrix, CapabilityState, Compensation, Diagnostic, Error,
    ErrorCode, Plan, PlanKind, PlanStep, PlanStepKind, Reason,
};
use serde::Serialize;
use thiserror as _;

fn assert_golden<T: Serialize>(value: &T, expected: &str) {
    let actual = format!("{}\n", serde_json::to_string_pretty(value).unwrap());
    assert_eq!(actual, expected);
}

#[test]
fn capability_matrix_matches_golden_contract() {
    let matrix = CapabilityMatrix {
        capabilities: vec![
            Capability {
                id: CapabilityId::Environment,
                state: CapabilityState::Available {
                    implementation: "platform_probe".into(),
                },
            },
            Capability {
                id: CapabilityId::RuntimeStart,
                state: CapabilityState::Unavailable {
                    reasons: vec![Reason::not_implemented()],
                },
            },
        ],
    };

    assert_golden(&matrix, include_str!("golden/capability-matrix.json"));
}

#[test]
fn plan_matches_golden_contract() {
    let plan = Plan {
        id: "create:phone_one".into(),
        kind: PlanKind::CreateDevice,
        steps: vec![PlanStep {
            id: "create_profile".into(),
            kind: PlanStepKind::ToolCall,
            description: "create temporary profile".into(),
            compensation: Some(Compensation {
                description: "delete temporary profile".into(),
            }),
        }],
    };

    assert_golden(&plan, include_str!("golden/plan.json"));
}

#[test]
fn error_matches_golden_contract() {
    let mut error = Error::new(
        ErrorCode::ToolOutputUnrecognized,
        "version output is invalid",
    );
    error.failed_step = Some("read_version".into());
    error.diagnostic = Some(Diagnostic {
        command: Some("android --version".into()),
        stdout: Some("unknown\n".into()),
        stderr: Some(String::new()),
        exit_status: Some(0),
    });

    assert_golden(&error, include_str!("golden/error.json"));
}
