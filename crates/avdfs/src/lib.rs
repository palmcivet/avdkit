//! File-level operations for AVD metadata.

#![allow(clippy::result_large_err)]

use std::{
    fmt::Write,
    fs, io,
    path::{Path, PathBuf},
};

use model::{AvdId, Error, ErrorCode, PackageId, PackageKind, ProfileId};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvdMetadata {
    pub id: AvdId,
    pub directory: PathBuf,
    pub display_name: Option<String>,
    pub profile: Option<ProfileId>,
    pub image: Option<PackageId>,
    pub target: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AvdStore {
    root: PathBuf,
}

impl AvdStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn list(&self) -> Result<Vec<AvdMetadata>, Error> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io_error("read AVD directory", &self.root, error)),
        };
        let mut indexes = entries
            .map(|entry| {
                entry
                    .map(|entry| entry.path())
                    .map_err(|error| io_error("read AVD directory entry", &self.root, error))
            })
            .collect::<Result<Vec<_>, _>>()?;
        indexes.retain(|path| path.extension().and_then(|value| value.to_str()) == Some("ini"));
        indexes.sort();

        let mut devices = Vec::new();
        for index in indexes {
            let Some(stem) = index.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            let Ok(id) = AvdId::new(stem) else {
                continue;
            };
            devices.push(self.read_index(id, &index)?);
        }
        devices.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        Ok(devices)
    }

    pub fn get(&self, id: &AvdId) -> Result<AvdMetadata, Error> {
        let index = self.root.join(format!("{id}.ini"));
        match fs::metadata(&index) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => {
                return Err(Error::new(
                    ErrorCode::PreconditionFailed,
                    format!("AVD index {} is not a file", index.display()),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(Error::new(
                    ErrorCode::DeviceNotFound,
                    format!("Android virtual device {id} was not found"),
                ));
            }
            Err(error) => return Err(io_error("inspect AVD index", &index, error)),
        }
        self.read_index(id.clone(), &index)
    }

    fn read_index(&self, id: AvdId, index_path: &Path) -> Result<AvdMetadata, Error> {
        let index = read_ini(index_path, "read AVD index")?;
        let directory = index
            .get("path")
            .map(PathBuf::from)
            .or_else(|| {
                index
                    .get("path.rel")
                    .map(|relative| self.root.parent().unwrap_or(&self.root).join(relative))
            })
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::PreconditionFailed,
                    format!("AVD index {} has no path", index_path.display()),
                )
            })?;

        let config_path = directory.join("config.ini");
        let config = match fs::read_to_string(&config_path) {
            Ok(contents) => Some(IniDocument::parse(&contents)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(io_error("read AVD config", &config_path, error)),
        };
        let display_name = config
            .as_ref()
            .and_then(|document| nonempty(document.get("avd.ini.displayname")));
        let profile = config
            .as_ref()
            .and_then(|document| nonempty(document.get("hw.device.name")))
            .and_then(|value| ProfileId::new(value).ok());
        let image = config
            .as_ref()
            .and_then(|document| document.get("image.sysdir.1"))
            .and_then(parse_system_image);
        let target = nonempty(index.get("target")).or_else(|| {
            config
                .as_ref()
                .and_then(|document| nonempty(document.get("target")))
        });

        Ok(AvdMetadata {
            id,
            directory,
            display_name,
            profile,
            image,
            target,
        })
    }
}

fn read_ini(path: &Path, action: &str) -> Result<IniDocument, Error> {
    fs::read_to_string(path)
        .map(|contents| IniDocument::parse(&contents))
        .map_err(|error| io_error(action, path, error))
}

fn nonempty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn parse_system_image(value: &str) -> Option<PackageId> {
    let normalized = value.trim().replace('\\', "/");
    let segments = normalized
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let start = segments
        .iter()
        .position(|segment| *segment == "system-images")?;
    let image = &segments[start..];
    if image.len() < 4 {
        return None;
    }
    Some(PackageId {
        kind: PackageKind::SystemImage,
        api: Some(image[1].strip_prefix("android-").unwrap_or(image[1]).into()),
        tag: Some(image[2].into()),
        abi: Some(image[3].into()),
        qualifier: (image.len() > 4).then(|| image[4..].join("/")),
    })
}

fn io_error(action: &str, path: &Path, error: io::Error) -> Error {
    Error::new(
        ErrorCode::Internal,
        format!("{action} {}: {error}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ROOT: AtomicU64 = AtomicU64::new(1);

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "avd-store-{}-{}",
            std::process::id(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn test_id(suffix: &str) -> String {
        format!("{}{suffix}", model::test_avd_prefix())
    }

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

    #[test]
    fn follows_index_path_and_reads_public_metadata() {
        let root = temp_root();
        let device_id = test_id("phone");
        let directory = root.join("elsewhere/device");
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            root.join(format!("{device_id}.ini")),
            format!("path={}\ntarget=android-36\n", directory.display()),
        )
        .unwrap();
        fs::write(
            directory.join("config.ini"),
            "avd.ini.displayname=Test Phone\n\
             hw.device.name=medium_phone\n\
             image.sysdir.1=system-images/android-36/google_apis/arm64-v8a/\n",
        )
        .unwrap();

        let device = AvdStore::new(&root)
            .get(&AvdId::new(&device_id).unwrap())
            .unwrap();
        assert_eq!(device.id.as_str(), device_id);
        assert_eq!(device.directory, directory);
        assert_eq!(device.display_name.as_deref(), Some("Test Phone"));
        assert_eq!(
            device.profile.as_ref().map(ProfileId::as_str),
            Some("medium_phone")
        );
        assert_eq!(
            device.image.as_ref().map(PackageId::render_slash),
            Some("system-images/android-36/google_apis/arm64-v8a".into())
        );
        assert_eq!(device.target.as_deref(), Some("android-36"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn list_is_sorted_and_missing_root_is_empty() {
        let root = temp_root();
        assert!(AvdStore::new(&root).list().unwrap().is_empty());
        fs::create_dir_all(&root).unwrap();
        for suffix in ["z", "a"] {
            let id = test_id(suffix);
            let directory = root.join(format!("{id}.avd"));
            fs::create_dir_all(&directory).unwrap();
            fs::write(
                root.join(format!("{id}.ini")),
                format!("path={}\n", directory.display()),
            )
            .unwrap();
        }
        fs::write(root.join("unrelated.txt"), "ignored").unwrap();

        let devices = AvdStore::new(&root).list().unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id.as_str(), test_id("a"));
        assert_eq!(devices[1].id.as_str(), test_id("z"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_device_has_a_stable_error() {
        let root = temp_root();
        let id = AvdId::new(test_id("missing")).unwrap();
        let error = AvdStore::new(root).get(&id).unwrap_err();
        assert_eq!(error.code, ErrorCode::DeviceNotFound);
    }

    #[test]
    fn index_without_a_path_fails_precondition() {
        let root = temp_root();
        let id = AvdId::new(test_id("broken")).unwrap();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(format!("{id}.ini")), "target=android-36\n").unwrap();

        let error = AvdStore::new(&root).get(&id).unwrap_err();
        assert_eq!(error.code, ErrorCode::PreconditionFailed);

        fs::remove_dir_all(root).unwrap();
    }
}
