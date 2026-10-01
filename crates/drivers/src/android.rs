use std::{path::PathBuf, str::FromStr};

use model::{Diagnostic, Error, ErrorCode, ProfileId, Revision};
use process::Output;

use crate::NormalizedOutput;

pub fn parse_version(output: &Output) -> Result<Revision, Error> {
    let normalized = NormalizedOutput::from(output);
    reject_known_error(output, &normalized)?;
    if normalized.status != Some(0) {
        return Err(unrecognized(output, "Android CLI version command failed"));
    }
    let Some(version) = normalized.stdout.first() else {
        return Err(unrecognized(output, "Android CLI version is missing"));
    };
    if normalized.stdout.len() != 1 {
        return Err(unrecognized(
            output,
            "Android CLI version output has unexpected lines",
        ));
    }
    Revision::from_str(version).map_err(|_| unrecognized(output, "Android CLI version is invalid"))
}

pub fn parse_sdk_root(output: &Output) -> Result<PathBuf, Error> {
    let normalized = NormalizedOutput::from(output);
    reject_known_error(output, &normalized)?;
    if normalized.status != Some(0) || normalized.stdout.len() != 1 {
        return Err(unrecognized(
            output,
            "Android CLI SDK root output is invalid",
        ));
    }
    let path = PathBuf::from(&normalized.stdout[0]);
    if !path.is_absolute() {
        return Err(unrecognized(output, "Android CLI SDK root is not absolute"));
    }
    Ok(path)
}

pub fn parse_profiles(output: &Output) -> Result<Vec<ProfileId>, Error> {
    let normalized = NormalizedOutput::from(output);
    reject_known_error(output, &normalized)?;
    if normalized.status != Some(0) || normalized.stdout.is_empty() {
        return Err(unrecognized(output, "Android CLI profile list is missing"));
    }
    normalized
        .stdout
        .iter()
        .map(|line| {
            if line.chars().any(char::is_whitespace) {
                return Err(unrecognized(
                    output,
                    "Android CLI profile identifier is invalid",
                ));
            }
            ProfileId::new(line.clone())
                .map_err(|_| unrecognized(output, "Android CLI profile identifier is invalid"))
        })
        .collect()
}

pub fn check_known_errors(output: &Output) -> Result<(), Error> {
    let normalized = NormalizedOutput::from(output);
    reject_known_error(output, &normalized)
}

pub fn require_success(output: &Output, action: &str) -> Result<(), Error> {
    check_known_errors(output)?;
    if output.status == Some(0) {
        Ok(())
    } else {
        Err(unrecognized(output, action))
    }
}

fn reject_known_error(output: &Output, normalized: &NormalizedOutput) -> Result<(), Error> {
    for line in normalized.lines() {
        let lowercase = line.to_ascii_lowercase();
        let classification = if lowercase.contains("terms of service")
            || lowercase.contains("first-run")
            || lowercase.contains("first run")
        {
            Some((
                ErrorCode::PreconditionFailed,
                "Android CLI first-run setup is incomplete",
            ))
        } else if line.contains("doesn't exist") {
            Some((
                ErrorCode::PreconditionFailed,
                "Android virtual device does not exist",
            ))
        } else if line.contains("already exists") {
            Some((
                ErrorCode::NameConflict,
                "Android virtual device already exists",
            ))
        } else if line.starts_with("Package ") && line.ends_with(" not found.") {
            Some((ErrorCode::PackageNotFound, "Android SDK package not found"))
        } else {
            None
        };
        if let Some((code, message)) = classification {
            return Err(error_with_diagnostic(output, code, message));
        }
    }
    Ok(())
}

fn unrecognized(output: &Output, message: &str) -> Error {
    error_with_diagnostic(output, ErrorCode::ToolOutputUnrecognized, message)
}

fn error_with_diagnostic(output: &Output, code: ErrorCode, message: &str) -> Error {
    let mut error = Error::new(code, message);
    error.diagnostic = Some(Box::new(Diagnostic {
        command: None,
        stdout: Some(output.stdout.clone()),
        stderr: Some(output.stderr.clone()),
        exit_status: output.status,
    }));
    error
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;

    #[test]
    fn replays_recorded_version_output() {
        let output = Fixture::load(include_str!(
            "../../../fixtures/macos-arm64/android/1.0.15985488/version.json"
        ))
        .output();
        assert_eq!(parse_version(&output).unwrap().components, [1, 0, 15985488]);
    }

    #[test]
    fn replays_recorded_profile_output() {
        let output = Fixture::load(include_str!(
            "../../../fixtures/macos-arm64/android/1.0.15985488/profiles.json"
        ))
        .output();
        let profiles = parse_profiles(&output).unwrap();
        assert_eq!(profiles.len(), 6);
        assert_eq!(profiles[2].as_str(), "medium_phone");
    }

    #[test]
    fn classifies_recorded_errors_even_when_exit_status_is_zero() {
        let output = Fixture::load(include_str!(
            "../../../fixtures/macos-arm64/android/1.0.15985488/missing-package.json"
        ))
        .output();
        assert_eq!(
            check_known_errors(&output).unwrap_err().code,
            ErrorCode::PackageNotFound
        );
    }

    #[test]
    fn unknown_output_is_not_treated_as_successful_parsed_data() {
        let output = Output {
            status: Some(0),
            stdout: "not a version\n".into(),
            stderr: String::new(),
        };
        assert_eq!(
            parse_version(&output).unwrap_err().code,
            ErrorCode::ToolOutputUnrecognized
        );
    }

    #[test]
    fn parses_an_absolute_sdk_root() {
        let output = Output {
            status: Some(0),
            stdout: "/opt/android-sdk\n".into(),
            stderr: String::new(),
        };
        assert_eq!(
            parse_sdk_root(&output).unwrap(),
            PathBuf::from("/opt/android-sdk")
        );
    }

    #[test]
    fn recognizes_incomplete_first_run_setup() {
        let output = Output {
            status: Some(0),
            stdout: "Android CLI Terms of Service must be accepted\n".into(),
            stderr: String::new(),
        };
        assert_eq!(
            parse_version(&output).unwrap_err().code,
            ErrorCode::PreconditionFailed
        );
    }
}
