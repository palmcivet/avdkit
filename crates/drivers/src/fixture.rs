use process::Output;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Fixture {
    #[serde(rename = "command")]
    pub _command: Vec<String>,
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Fixture {
    pub fn load(json: &str) -> Self {
        serde_json::from_str(json).unwrap()
    }

    pub fn output(self) -> Output {
        Output {
            status: Some(self.status),
            stdout: self.stdout,
            stderr: self.stderr,
        }
    }
}
