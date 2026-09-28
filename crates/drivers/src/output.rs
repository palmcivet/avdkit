use process::Output;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedOutput {
    pub status: Option<i32>,
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
}

impl From<&Output> for NormalizedOutput {
    fn from(output: &Output) -> Self {
        Self {
            status: output.status,
            stdout: clean_stream(&output.stdout),
            stderr: clean_stream(&output.stderr),
        }
    }
}

impl NormalizedOutput {
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.stdout.iter().chain(&self.stderr).map(String::as_str)
    }
}

fn clean_stream(stream: &str) -> Vec<String> {
    strip_ansi(stream)
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with("Warning:"))
        .filter(|line| !is_update_notice(line))
        .map(str::to_owned)
        .collect()
}

fn is_update_notice(line: &str) -> bool {
    line.starts_with("A new version of Android CLI is available")
        || line.starts_with("To update Android CLI")
}

fn strip_ansi(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            result.push(character);
            continue;
        }
        if characters.next() != Some('[') {
            continue;
        }
        for parameter in characters.by_ref() {
            if ('@'..='~').contains(&parameter) {
                break;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_ansi_warnings_updates_and_carriage_return_progress() {
        let output = Output {
            status: Some(0),
            stdout: "\u{1b}[32mfirst\u{1b}[0m\rsecond\nWarning: ignored\n\
                A new version of Android CLI is available\n"
                .into(),
            stderr: "third\n".into(),
        };
        let normalized = NormalizedOutput::from(&output);
        assert_eq!(normalized.stdout, ["first", "second"]);
        assert_eq!(normalized.stderr, ["third"]);
    }
}
