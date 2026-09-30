use model::{
    Compensation, CreateDeviceDraft, Error, ErrorCode, Plan, PlanIntent, PlanKind, PlanStep,
    PlanStepKind,
};

pub fn compile_create(draft: &CreateDeviceDraft) -> Plan {
    let display = draft
        .display_name
        .clone()
        .unwrap_or_else(|| draft.id.as_str().to_owned());
    Plan {
        id: format!("create:{}", draft.id),
        kind: PlanKind::CreateDevice,
        intent: PlanIntent::CreateDevice {
            draft: draft.clone(),
        },
        steps: vec![
            PlanStep {
                id: "create".into(),
                kind: PlanStepKind::ToolCall,
                description: format!("create an AVD from profile {}", draft.profile),
                compensation: Some(Compensation {
                    description: "remove the newly created AVD".into(),
                }),
            },
            PlanStep {
                id: "rename".into(),
                kind: PlanStepKind::FileRewrite,
                description: format!("move the AVD to {}", draft.id),
                compensation: None,
            },
            PlanStep {
                id: "image".into(),
                kind: PlanStepKind::FileRewrite,
                description: format!("point the AVD at {}", draft.image.render_semicolon()),
                compensation: None,
            },
            PlanStep {
                id: "display_name".into(),
                kind: PlanStepKind::FileRewrite,
                description: format!("set display name to {display}"),
                compensation: None,
            },
        ],
    }
}

/// Recovers the draft of an approved plan.
///
/// Only the structure that determines execution is compared with a fresh
/// compilation, so plans saved before a wording change remain executable.
pub fn create_draft(plan: &Plan) -> Result<CreateDeviceDraft, Error> {
    let PlanIntent::CreateDevice { draft } = &plan.intent else {
        return Err(Error::not_implemented("this plan intent"));
    };
    if !draft.hardware.is_default() {
        return Err(Error::not_implemented("custom hardware"));
    }
    let expected = compile_create(draft);
    let same_structure = plan.id == expected.id
        && plan.kind == expected.kind
        && plan.steps.len() == expected.steps.len()
        && plan
            .steps
            .iter()
            .zip(&expected.steps)
            .all(|(step, expected)| {
                step.id == expected.id
                    && step.kind == expected.kind
                    && step.compensation.is_some() == expected.compensation.is_some()
            });
    if !same_structure {
        return Err(Error::new(
            ErrorCode::InvalidInput,
            "plan structure does not match its compiled intent",
        ));
    }
    Ok(draft.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::{AvdId, HardwareConfig, PackageId, PackageKind, ProfileId};

    fn draft() -> CreateDeviceDraft {
        CreateDeviceDraft {
            id: AvdId::new("phone").unwrap(),
            profile: ProfileId::new("medium_phone").unwrap(),
            image: PackageId {
                kind: PackageKind::SystemImage,
                api: Some("36".into()),
                tag: Some("google_apis".into()),
                abi: Some("arm64-v8a".into()),
                qualifier: None,
            },
            display_name: None,
            hardware: HardwareConfig::default(),
        }
    }

    #[test]
    fn reworded_descriptions_remain_executable() {
        let mut plan = compile_create(&draft());
        plan.steps[0].description = "wording from another library version".into();
        assert_eq!(create_draft(&plan).unwrap(), draft());
    }

    #[test]
    fn structural_changes_are_rejected() {
        let mut plan = compile_create(&draft());
        plan.steps.swap(1, 2);
        assert_eq!(
            create_draft(&plan).unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
}
