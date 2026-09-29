//! File-level operations for AVD metadata.

#![allow(clippy::result_large_err)]

use std::{
    fmt::Write,
    fs,
    fs::OpenOptions,
    io::{self, Write as IoWrite},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use model::{AvdId, Error, ErrorCode, PackageId, PackageKind, ProfileId};

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(1);

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
    user_root: PathBuf,
}

impl AvdStore {
    pub fn new(root: impl Into<PathBuf>, user_root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            user_root: user_root.into(),
        }
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

    pub fn contains_paths(&self, id: &AvdId) -> Result<bool, Error> {
        let index = self.root.join(format!("{id}.ini"));
        let directory = self.root.join(format!("{id}.avd"));
        path_exists(&index).and_then(|index_exists| {
            path_exists(&directory).map(|directory_exists| index_exists || directory_exists)
        })
    }

    fn read_index(&self, id: AvdId, index_path: &Path) -> Result<AvdMetadata, Error> {
        let index = read_ini(index_path, "read AVD index")?;
        let directory = index
            .get("path")
            .map(PathBuf::from)
            .or_else(|| {
                index
                    .get("path.rel")
                    .map(|relative| self.user_root.join(relative))
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

#[derive(Debug)]
pub struct CreateTransaction {
    root: PathBuf,
    source_id: AvdId,
    target_id: AvdId,
    source_directory: PathBuf,
    target_directory: PathBuf,
    backup_root: PathBuf,
    moved: bool,
}

impl CreateTransaction {
    pub fn begin(
        root: impl Into<PathBuf>,
        user_root: impl Into<PathBuf>,
        backup_root: impl Into<PathBuf>,
        source_id: AvdId,
        target_id: AvdId,
    ) -> Result<Self, Error> {
        let root = root.into();
        let user_root = user_root.into();
        let backup_root = backup_root.into();
        let source_index = root.join(format!("{source_id}.ini"));
        let index = read_ini(&source_index, "read newly created AVD index")?;
        let source_directory = index
            .get("path")
            .map(PathBuf::from)
            .or_else(|| index.get("path.rel").map(|path| user_root.join(path)))
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::PreconditionFailed,
                    format!("new AVD index {} has no path", source_index.display()),
                )
            })?;
        let expected_directory = root.join(format!("{source_id}.avd"));
        if source_directory != expected_directory {
            return Err(Error::new(
                ErrorCode::PreconditionFailed,
                format!(
                    "new AVD directory {} did not match expected path {}",
                    source_directory.display(),
                    expected_directory.display()
                ),
            ));
        }
        let source_config = source_directory.join("config.ini");
        if !source_config.is_file() {
            return Err(Error::new(
                ErrorCode::PreconditionFailed,
                format!("new AVD config {} was not created", source_config.display()),
            ));
        }

        fs::create_dir_all(&backup_root).map_err(|error| {
            io_error("create transaction backup directory", &backup_root, error)
        })?;
        let backup_result = fs::copy(&source_index, backup_root.join("index.ini"))
            .map_err(|error| io_error("back up AVD index", &source_index, error))
            .and_then(|_| {
                fs::copy(&source_config, backup_root.join("config.ini"))
                    .map_err(|error| io_error("back up AVD config", &source_config, error))
            });
        if let Err(error) = backup_result {
            let _ = fs::remove_dir_all(&backup_root);
            return Err(error);
        }

        let target_directory = root.join(format!("{target_id}.avd"));
        Ok(Self {
            root,
            source_id,
            target_id,
            source_directory,
            target_directory,
            backup_root,
            moved: false,
        })
    }

    pub fn rename(&mut self) -> Result<(), Error> {
        if self.source_id != self.target_id {
            fs::rename(&self.source_directory, &self.target_directory).map_err(|error| {
                io_error(
                    "move temporary AVD directory",
                    &self.source_directory,
                    error,
                )
            })?;
            let source_index = self.source_index();
            if let Err(error) = fs::rename(&source_index, self.target_index()) {
                let _ = fs::rename(&self.target_directory, &self.source_directory);
                return Err(io_error("move temporary AVD index", &source_index, error));
            }
        }
        self.moved = true;

        let mut index = read_ini(&self.target_index(), "read moved AVD index")?;
        index.set("path", &self.target_directory.to_string_lossy());
        if let Some(relative) = index.get("path.rel").map(str::to_owned) {
            let replacement = Path::new(&relative)
                .parent()
                .map_or_else(
                    || PathBuf::from(format!("{}.avd", self.target_id)),
                    |parent| parent.join(format!("{}.avd", self.target_id)),
                )
                .to_string_lossy()
                .into_owned();
            index.set("path.rel", &replacement);
        }
        atomic_write(&self.target_index(), &index.render())?;

        let config_path = self.target_directory.join("config.ini");
        let mut config = read_ini(&config_path, "read moved AVD config")?;
        config.set("AvdId", self.target_id.as_str());
        atomic_write(&config_path, &config.render())
    }

    pub fn set_image(&mut self, image: &PackageId) -> Result<(), Error> {
        let (target, tag, tag_display) = image_values(image)?;
        let config_path = self.target_directory.join("config.ini");
        let mut config = read_ini(&config_path, "read AVD config for image rewrite")?;
        config.set("image.sysdir.1", &format!("{}/", image.render_slash()));
        config.set("tag.id", tag);
        config.set("tag.ids", tag);
        config.set("tag.display", &tag_display);
        config.set("tag.displaynames", &tag_display);
        config.set(
            "PlayStore.enabled",
            if tag.contains("playstore") {
                "true"
            } else {
                "false"
            },
        );
        config.set("target", &target);
        atomic_write(&config_path, &config.render())?;

        let index_path = self.target_index();
        let mut index = read_ini(&index_path, "read AVD index for target rewrite")?;
        index.set("target", &target);
        atomic_write(&index_path, &index.render())
    }

    pub fn set_display_name(&mut self, display_name: &str) -> Result<(), Error> {
        let config_path = self.target_directory.join("config.ini");
        let mut config = read_ini(&config_path, "read AVD config for display name rewrite")?;
        config.set("avd.ini.displayname", display_name);
        atomic_write(&config_path, &config.render())
    }

    pub fn compensate(&mut self) -> Result<(), Error> {
        let mut failures = Vec::new();
        if self.moved {
            if let Err(error) = self.restore_backups() {
                failures.push(error.message);
            }
        }
        for path in [
            self.target_index(),
            self.source_index(),
            self.target_directory.clone(),
            self.source_directory.clone(),
        ] {
            if let Err(error) = remove_path_if_present(&path) {
                failures.push(error.message);
            }
        }
        if let Err(error) = remove_path_if_present(&self.backup_root) {
            failures.push(error.message);
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(Error::new(ErrorCode::Internal, failures.join("; ")))
        }
    }

    pub fn commit(&mut self) -> Result<(), Error> {
        remove_path_if_present(&self.backup_root)
    }

    fn restore_backups(&self) -> Result<(), Error> {
        let index_path = self.target_index();
        let config_path = self.target_directory.join("config.ini");
        let index = fs::read_to_string(self.backup_root.join("index.ini"))
            .map_err(|error| io_error("read AVD index backup", &self.backup_root, error))?;
        let config = fs::read_to_string(self.backup_root.join("config.ini"))
            .map_err(|error| io_error("read AVD config backup", &self.backup_root, error))?;
        atomic_write(&index_path, &index)?;
        atomic_write(&config_path, &config)
    }

    fn source_index(&self) -> PathBuf {
        self.root.join(format!("{}.ini", self.source_id))
    }

    fn target_index(&self) -> PathBuf {
        self.root.join(format!("{}.ini", self.target_id))
    }
}

pub fn remove_created_artifacts(root: &Path, id: &AvdId) -> Result<(), Error> {
    let mut failures = Vec::new();
    for path in [
        root.join(format!("{id}.ini")),
        root.join(format!("{id}.avd")),
    ] {
        if let Err(error) = remove_path_if_present(&path) {
            failures.push(error.message);
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(Error::new(ErrorCode::Internal, failures.join("; ")))
    }
}

fn read_ini(path: &Path, action: &str) -> Result<IniDocument, Error> {
    fs::read_to_string(path)
        .map(|contents| IniDocument::parse(&contents))
        .map_err(|error| io_error(action, path, error))
}

fn image_values(image: &PackageId) -> Result<(String, &str, String), Error> {
    if image.kind != PackageKind::SystemImage {
        return Err(Error::new(
            ErrorCode::InvalidInput,
            "device image must be a system image",
        ));
    }
    let api = image
        .api
        .as_deref()
        .ok_or_else(|| Error::new(ErrorCode::InvalidInput, "device image API is missing"))?;
    let tag = image
        .tag
        .as_deref()
        .ok_or_else(|| Error::new(ErrorCode::InvalidInput, "device image tag is missing"))?;
    if image.abi.is_none() {
        return Err(Error::new(
            ErrorCode::InvalidInput,
            "device image ABI is missing",
        ));
    }
    let target = if api.starts_with("android-") {
        api.to_owned()
    } else {
        format!("android-{api}")
    };
    let display = match tag {
        "google_apis_playstore" => "Google Play".into(),
        "google_apis" => "Google APIs".into(),
        "default" => "Default Android System Image".into(),
        _ => tag
            .split('_')
            .map(|part| {
                let mut characters = part.chars();
                characters.next().map_or_else(String::new, |first| {
                    first.to_uppercase().collect::<String>() + characters.as_str()
                })
            })
            .collect::<Vec<_>>()
            .join(" "),
    };
    Ok((target, tag, display))
}

fn atomic_write(path: &Path, contents: &str) -> Result<(), Error> {
    let parent = path.parent().ok_or_else(|| {
        Error::new(
            ErrorCode::Internal,
            format!("file {} has no parent directory", path.display()),
        )
    })?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        model::PRODUCT_NAME,
        NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| io_error("create temporary AVD file", &temporary, error))?;
        file.write_all(contents.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| io_error("write temporary AVD file", &temporary, error))?;
        fs::rename(&temporary, path).map_err(|error| io_error("replace AVD file", path, error))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn path_exists(path: &Path) -> Result<bool, Error> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error("inspect AVD path", path, error)),
    }
}

fn remove_path_if_present(path: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(path).map_err(|error| io_error("remove AVD directory", path, error))
        }
        Ok(_) => fs::remove_file(path).map_err(|error| io_error("remove AVD file", path, error)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error("inspect AVD path for removal", path, error)),
    }
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

        let device = AvdStore::new(&root, &root)
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
        assert!(AvdStore::new(&root, &root).list().unwrap().is_empty());
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

        let devices = AvdStore::new(&root, &root).list().unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id.as_str(), test_id("a"));
        assert_eq!(devices[1].id.as_str(), test_id("z"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_device_has_a_stable_error() {
        let root = temp_root();
        let id = AvdId::new(test_id("missing")).unwrap();
        let error = AvdStore::new(&root, &root).get(&id).unwrap_err();
        assert_eq!(error.code, ErrorCode::DeviceNotFound);
    }

    #[test]
    fn index_without_a_path_fails_precondition() {
        let root = temp_root();
        let id = AvdId::new(test_id("broken")).unwrap();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(format!("{id}.ini")), "target=android-36\n").unwrap();

        let error = AvdStore::new(&root, &root).get(&id).unwrap_err();
        assert_eq!(error.code, ErrorCode::PreconditionFailed);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_relative_paths_from_the_android_user_directory() {
        let base = temp_root();
        let avd_root = base.join("custom-avd-home");
        let user_root = base.join("android-user-home");
        let device_id = test_id("relative");
        let directory = user_root.join("avd/relative.avd");
        fs::create_dir_all(&avd_root).unwrap();
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            avd_root.join(format!("{device_id}.ini")),
            "path.rel=avd/relative.avd\n",
        )
        .unwrap();
        fs::write(directory.join("config.ini"), "target=android-36\n").unwrap();

        let device = AvdStore::new(&avd_root, &user_root)
            .get(&AvdId::new(&device_id).unwrap())
            .unwrap();
        assert_eq!(device.directory, directory);
        assert_eq!(device.target.as_deref(), Some("android-36"));

        fs::remove_dir_all(base).unwrap();
    }
}
