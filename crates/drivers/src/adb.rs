use std::collections::BTreeMap;

use model::{AvdId, Diagnostic, Error, ErrorCode};
use process::Output;

use crate::NormalizedOutput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub serial: String,
    pub state: String,
    pub details: BTreeMap<String, String>,
}

pub fn parse_devices(output: &Output) -> Result<Vec<Device>, Error> {
    let normalized = NormalizedOutput::from(output);
    if normalized.status != Some(0) {
        return Err(unrecognized(output, "adb failed to list devices"));
    }
    let Some(header) = normalized
        .stdout
        .iter()
        .position(|line| line == "List of devices attached")
    else {
        return Err(unrecognized(output, "adb device list header is missing"));
    };

    normalized.stdout[header + 1..]
        .iter()
        .filter(|line| !line.starts_with("* daemon"))
        .map(|line| parse_device(output, line))
        .collect()
}

pub fn parse_avd_name(output: &Output) -> Result<Option<AvdId>, Error> {
    if output.status != Some(0) {
        return Ok(None);
    }
    let normalized = NormalizedOutput::from(output);
    let name = normalized
        .stdout
        .iter()
        .map(String::as_str)
        .find(|line| !line.eq_ignore_ascii_case("ok") && !line.eq_ignore_ascii_case("unknown"));
    name.map(|name| {
        AvdId::new(name.to_owned())
            .map_err(|_| unrecognized(output, "adb returned an invalid AVD identifier"))
    })
    .transpose()
}

pub fn boot_completed(output: &Output) -> bool {
    output.status == Some(0)
        && NormalizedOutput::from(output)
            .stdout
            .iter()
            .any(|line| line == "1")
}

pub fn package_manager_ready(output: &Output) -> bool {
    output.status == Some(0)
        && NormalizedOutput::from(output)
            .stdout
            .iter()
            .any(|line| line.starts_with("package:"))
}

pub fn emu_kill_succeeded(output: &Output) -> bool {
    output.status == Some(0)
        && NormalizedOutput::from(output).lines().any(|line| {
            line.eq_ignore_ascii_case("ok")
                || line.to_ascii_lowercase().contains("killing emulator")
        })
}

fn parse_device(output: &Output, line: &str) -> Result<Device, Error> {
    let mut columns = line.split_whitespace();
    let serial = columns
        .next()
        .ok_or_else(|| unrecognized(output, "adb device serial is missing"))?;
    let state = columns
        .next()
        .ok_or_else(|| unrecognized(output, "adb device state is missing"))?;
    let details = columns
        .filter_map(|column| column.split_once(':'))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect();
    Ok(Device {
        serial: serial.into(),
        state: state.into(),
        details,
    })
}

fn unrecognized(output: &Output, message: &str) -> Error {
    let mut error = Error::new(ErrorCode::ToolOutputUnrecognized, message);
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
    fn replays_recorded_empty_device_list() {
        let output = Fixture::load(include_str!(
            "../../../fixtures/macos-arm64/adb/37.0.1/devices-empty.json"
        ))
        .output();
        assert!(parse_devices(&output).unwrap().is_empty());
    }

    #[test]
    fn parses_states_and_key_value_details() {
        let output = Output {
            status: Some(0),
            stdout: "List of devices attached\n\
                emulator-5554 device product:sdk_phone model:Phone transport_id:1\n\
                emulator-5556 offline transport_id:2\n"
                .into(),
            stderr: String::new(),
        };
        let devices = parse_devices(&output).unwrap();
        assert_eq!(devices[0].serial, "emulator-5554");
        assert_eq!(devices[0].state, "device");
        assert_eq!(devices[0].details["model"], "Phone");
        assert_eq!(devices[1].state, "offline");
    }

    #[test]
    fn rejects_unrecognized_output() {
        let output = Output {
            status: Some(0),
            stdout: "unexpected\n".into(),
            stderr: String::new(),
        };
        assert_eq!(
            parse_devices(&output).unwrap_err().code,
            ErrorCode::ToolOutputUnrecognized
        );
    }

    #[test]
    fn parses_property_and_console_avd_names() {
        let property = Output {
            status: Some(0),
            stdout: "avdkit_test_phone\n".into(),
            stderr: String::new(),
        };
        assert_eq!(
            parse_avd_name(&property).unwrap().unwrap().as_str(),
            "avdkit_test_phone"
        );
        let console = Output {
            status: Some(0),
            stdout: "avdkit_test_phone\nOK\n".into(),
            stderr: String::new(),
        };
        assert_eq!(
            parse_avd_name(&console).unwrap().unwrap().as_str(),
            "avdkit_test_phone"
        );
    }

    #[test]
    fn requires_both_boot_and_package_manager_signals() {
        let boot = Output {
            status: Some(0),
            stdout: "1\n".into(),
            stderr: String::new(),
        };
        let packages = Output {
            status: Some(0),
            stdout: "package:/system/framework/framework-res.apk\n".into(),
            stderr: String::new(),
        };
        assert!(boot_completed(&boot));
        assert!(package_manager_ready(&packages));
    }
}
