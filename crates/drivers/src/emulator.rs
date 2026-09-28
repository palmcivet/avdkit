use model::{AvdId, Diagnostic, Error, ErrorCode};
use process::Output;

use crate::NormalizedOutput;

pub fn parse_avd_ids(output: &Output) -> Result<Vec<AvdId>, Error> {
    let normalized = NormalizedOutput::from(output);
    if normalized.status != Some(0) {
        return Err(unrecognized(output, "emulator failed to list AVDs"));
    }

    normalized
        .stdout
        .iter()
        .filter(|line| looks_like_avd_id(line))
        .map(|line| {
            AvdId::new(line.clone())
                .map_err(|_| unrecognized(output, "emulator returned an invalid AVD identifier"))
        })
        .collect()
}

fn looks_like_avd_id(line: &str) -> bool {
    !line.is_empty()
        && !line.chars().any(char::is_whitespace)
        && !line.starts_with("INFO")
        && !line.starts_with("WARNING")
        && !line.starts_with("ERROR")
}

fn unrecognized(output: &Output, message: &str) -> Error {
    let mut error = Error::new(ErrorCode::ToolOutputUnrecognized, message);
    error.diagnostic = Some(Diagnostic {
        command: None,
        stdout: Some(output.stdout.clone()),
        stderr: Some(output.stderr.clone()),
        exit_status: output.status,
    });
    error
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;

    #[test]
    fn replays_recorded_avd_names() {
        let output = Fixture::load(include_str!(
            "../../../fixtures/macos-arm64/emulator/37.1.11/list-avds.json"
        ))
        .output();
        let ids = parse_avd_ids(&output).unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].as_str(), "avdkit_test_fixture");
    }

    #[test]
    fn filters_emulator_log_lines_from_avd_names() {
        let output = Output {
            status: Some(0),
            stdout: "INFO | emulator startup\navdkit_test_phone\n".into(),
            stderr: String::new(),
        };
        assert_eq!(parse_avd_ids(&output).unwrap().len(), 1);
    }

    #[test]
    fn accepts_an_empty_avd_list() {
        let output = Output {
            status: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        };
        assert!(parse_avd_ids(&output).unwrap().is_empty());
    }
}
