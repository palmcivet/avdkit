use avdkit_model::{
    Capability, CapabilityId, CapabilityMatrix, CapabilityState, Compensation, Diagnostic, Error,
    ErrorCode, HardwareConfig, PackageId, PackageKind, Plan, PlanIntent, PlanKind, PlanStep,
    PlanStepKind, ProfileId, Reason,
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
        intent: PlanIntent::CreateDevice {
            draft: avdkit_model::CreateDeviceDraft {
                id: avdkit_model::AvdId::new("phone_one").unwrap(),
                profile: ProfileId::new("medium_phone").unwrap(),
                image: PackageId {
                    kind: PackageKind::SystemImage,
                    api: Some("36".into()),
                    tag: Some("google_apis".into()),
                    abi: Some("arm64-v8a".into()),
                    qualifier: None,
                },
                display_name: Some("Phone One".into()),
                hardware: HardwareConfig::default(),
            },
        },
        steps: vec![
            PlanStep {
                id: "create".into(),
                kind: PlanStepKind::ToolCall,
                description: "create an AVD from profile medium_phone".into(),
                compensation: Some(Compensation {
                    description: "remove the newly created AVD".into(),
                }),
            },
            PlanStep {
                id: "rename".into(),
                kind: PlanStepKind::FileRewrite,
                description: "move the AVD to phone_one".into(),
                compensation: None,
            },
            PlanStep {
                id: "image".into(),
                kind: PlanStepKind::FileRewrite,
                description: "point the AVD at system-images;android-36;google_apis;arm64-v8a"
                    .into(),
                compensation: None,
            },
            PlanStep {
                id: "display_name".into(),
                kind: PlanStepKind::FileRewrite,
                description: "set display name to Phone One".into(),
                compensation: None,
            },
        ],
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
