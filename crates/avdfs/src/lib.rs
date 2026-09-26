//! File-level operations for AVD metadata.

use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    Raw(String),
    KeyValue { key: String, value: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IniDocument {
    lines: Vec<Line>,
}

impl IniDocument {
    pub fn parse(input: &str) -> Self {
        let lines = input
            .lines()
            .map(|line| {
                line.split_once('=').map_or_else(
                    || Line::Raw(line.to_owned()),
                    |(key, value)| Line::KeyValue {
                        key: key.to_owned(),
                        value: value.to_owned(),
                    },
                )
            })
            .collect();
        Self { lines }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().find_map(|line| match line {
            Line::KeyValue {
                key: line_key,
                value,
            } if line_key == key => Some(value.as_str()),
            _ => None,
        })
    }

    pub fn set(&mut self, key: &str, value: &str) {
        if let Some(Line::KeyValue {
            value: line_value, ..
        }) = self
            .lines
            .iter_mut()
            .find(|line| matches!(line, Line::KeyValue { key: line_key, .. } if line_key == key))
        {
            *line_value = value.to_owned();
        } else {
            self.lines.push(Line::KeyValue {
                key: key.to_owned(),
                value: value.to_owned(),
            });
        }
    }

    pub fn render(&self) -> String {
        let mut output = String::new();
        for (index, line) in self.lines.iter().enumerate() {
            if index > 0 {
                output.push('\n');
            }
            match line {
                Line::Raw(value) => output.push_str(value),
                Line::KeyValue { key, value } => {
                    let _ = write!(output, "{key}={value}");
                }
            }
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_unknown_lines_and_values_containing_equals() {
        let mut document = IniDocument::parse("# comment\npath=/tmp/a=b\nunknown=value");
        assert_eq!(document.get("path"), Some("/tmp/a=b"));
        document.set("path", "/tmp/changed");
        document.set("new", "value");
        assert_eq!(
            document.render(),
            "# comment\npath=/tmp/changed\nunknown=value\nnew=value"
        );
    }
}
