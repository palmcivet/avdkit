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

pub fn create_draft(plan: &Plan) -> Result<CreateDeviceDraft, Error> {
    let PlanIntent::CreateDevice { draft } = &plan.intent;
    if !draft.hardware.is_default() {
        return Err(Error::not_implemented("custom hardware"));
    }
    let expected = compile_create(draft);
    if plan != &expected {
        return Err(Error::new(
            ErrorCode::InvalidInput,
            "plan contents do not match its compiled intent",
        ));
    }
    Ok(draft.clone())
}
