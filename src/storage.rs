use std::{
    env,
    ffi::OsString,
    fmt::Write as _,
    fs::{self, File},
    io::{ErrorKind, Write as _},
    path::{Path, PathBuf},
    process,
};

use anyhow::{Context, Result, anyhow, bail};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::{Store, Todo};

pub enum DataLocation {
    Directory(PathBuf),
    JsonFile(PathBuf),
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Metadata {
    #[serde(default)]
    next_id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DayFile {
    date: NaiveDate,
    #[serde(default)]
    todos: Vec<Todo>,
}

pub fn data_location() -> Result<DataLocation> {
    if let Some(path) = nonempty_env("TS_DATA_FILE") {
        return Ok(DataLocation::JsonFile(PathBuf::from(path)));
    }

    if let Some(path) = nonempty_env("TS_DATA_DIR") {
        return Ok(DataLocation::Directory(PathBuf::from(path)));
    }

    if let Some(path) = nonempty_env("XDG_CONFIG_HOME") {
        return Ok(DataLocation::Directory(PathBuf::from(path).join("ts")));
    }

    if let Some(home) = nonempty_env("HOME") {
        return Ok(DataLocation::Directory(
            PathBuf::from(home).join(".config").join("ts"),
        ));
    }

    Err(anyhow!(
        "Could not determine a config directory. Set TS_DATA_DIR to a writable directory."
    ))
}

pub fn load(location: &DataLocation) -> Result<Store> {
    match location {
        DataLocation::Directory(root) => load_daily(root),
        DataLocation::JsonFile(path) => match fs::read(path) {
            Ok(bytes) => decode_json(path, &bytes),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Store::default()),
            Err(error) => Err(error).with_context(|| format!("Could not read {}", path.display())),
        },
    }
}

pub fn save_day(location: &DataLocation, store: &Store, date: NaiveDate) -> Result<()> {
    match location {
        DataLocation::Directory(root) => save_daily_day(root, store, date),
        DataLocation::JsonFile(path) => save_json(path, store),
    }
}

fn load_daily(root: &Path) -> Result<Store> {
    let metadata_path = root.join("metadata.toml");
    let next_id = match fs::read_to_string(&metadata_path) {
        Ok(contents) => {
            toml::from_str::<Metadata>(&contents)
                .with_context(|| format!("Could not parse {}", metadata_path.display()))?
                .next_id
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            if let Some(store) = migrate_legacy(root)? {
                return Ok(store);
            }
            1
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Could not read {}", metadata_path.display()));
        }
    };

    let mut store = Store {
        next_id,
        days: Default::default(),
    };

    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(store),
        Err(error) => {
            return Err(error).with_context(|| format!("Could not read {}", root.display()));
        }
    };

    for entry in entries {
        let entry = entry.with_context(|| format!("Could not read {}", root.display()))?;
        if !entry
            .file_type()
            .with_context(|| format!("Could not inspect {}", entry.path().display()))?
            .is_dir()
        {
            continue;
        }
        load_month(root, &entry.path(), &mut store)?;
    }

    store.normalize_next_id();
    Ok(store)
}

fn load_month(root: &Path, month_path: &Path, store: &mut Store) -> Result<()> {
    for entry in fs::read_dir(month_path)
        .with_context(|| format!("Could not read {}", month_path.display()))?
    {
        let entry = entry.with_context(|| format!("Could not read {}", month_path.display()))?;
        let path = entry.path();
        if !entry
            .file_type()
            .with_context(|| format!("Could not inspect {}", path.display()))?
            .is_file()
            || path.extension().and_then(|extension| extension.to_str()) != Some("toml")
        {
            continue;
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Could not read {}", path.display()))?;
        let day: DayFile = toml::from_str(&contents)
            .with_context(|| format!("Could not parse todo data in {}", path.display()))?;
        let expected_path = day_path(root, day.date);
        if path != expected_path {
            bail!(
                "Todo file {} does not match its date {}; expected {}",
                path.display(),
                day.date,
                expected_path.display()
            );
        }
        if store.days.insert(day.date, day.todos).is_some() {
            bail!("Duplicate todo data for {} in {}", day.date, path.display());
        }
    }
    Ok(())
}

fn migrate_legacy(root: &Path) -> Result<Option<Store>> {
    let mut paths = vec![root.join("todos.json")];
    if let Some(data_home) = nonempty_env("XDG_DATA_HOME") {
        paths.push(PathBuf::from(data_home).join("ts").join("todos.json"));
    } else if let Some(home) = nonempty_env("HOME") {
        paths.push(
            PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("ts")
                .join("todos.json"),
        );
    }

    for path in paths {
        match fs::read(&path) {
            Ok(bytes) => {
                let store = decode_json(&path, &bytes)?;
                save_daily(root, &store).with_context(|| {
                    format!(
                        "Could not migrate todo data from {} to {}",
                        path.display(),
                        root.display()
                    )
                })?;
                return Ok(Some(store));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("Could not read {}", path.display()));
            }
        }
    }
    Ok(None)
}

fn decode_json(path: &Path, bytes: &[u8]) -> Result<Store> {
    let mut store: Store = serde_json::from_slice(bytes)
        .with_context(|| format!("Could not parse todo data in {}", path.display()))?;
    store.normalize_next_id();
    Ok(store)
}

fn save_json(path: &Path, store: &Store) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(store).context("Could not encode todo data")?;
    write_atomic(path, &bytes)
}

fn save_daily(root: &Path, store: &Store) -> Result<()> {
    for (date, todos) in &store.days {
        write_day(root, *date, todos)?;
    }
    write_metadata(root, store.next_id)
}

fn save_daily_day(root: &Path, store: &Store, date: NaiveDate) -> Result<()> {
    if let Some(todos) = store.days.get(&date).filter(|todos| !todos.is_empty()) {
        write_day(root, date, todos)?;
    } else {
        let path = day_path(root, date);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("Could not remove {}", path.display()));
            }
        }
    }
    write_metadata(root, store.next_id)
}

fn write_day(root: &Path, date: NaiveDate, todos: &[Todo]) -> Result<()> {
    let day = DayFile {
        date,
        todos: todos.to_vec(),
    };
    let contents = toml::to_string_pretty(&day).context("Could not encode todo data")?;
    write_atomic(&day_path(root, date), contents.as_bytes())
}

fn write_metadata(root: &Path, next_id: u64) -> Result<()> {
    let contents =
        toml::to_string_pretty(&Metadata { next_id }).context("Could not encode todo metadata")?;
    write_atomic(&root.join("metadata.toml"), contents.as_bytes())
}

fn day_path(root: &Path, date: NaiveDate) -> PathBuf {
    root.join(hash16(&date.format("%Y-%m").to_string()))
        .join(format!(
            "{}.toml",
            hash16(&date.format("%Y-%m-%d").to_string())
        ))
}

fn hash16(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest[..8]
        .iter()
        .fold(String::with_capacity(16), |mut hash, byte| {
            write!(hash, "{byte:02x}").expect("writing to a string cannot fail");
            hash
        })
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("Could not create {}", parent.display()))?;
    }

    let mut temporary_name: OsString = path.as_os_str().to_owned();
    temporary_name.push(format!(".tmp-{}", process::id()));
    let temporary_path = PathBuf::from(temporary_name);

    let result = (|| -> Result<()> {
        let mut file = File::create(&temporary_path)
            .with_context(|| format!("Could not create {}", temporary_path.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("Could not write {}", temporary_path.display()))?;
        file.sync_all()
            .with_context(|| format!("Could not flush {}", temporary_path.display()))?;
        fs::rename(&temporary_path, path)
            .with_context(|| format!("Could not replace {}", path.display()))?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn nonempty_env(name: &str) -> Option<OsString> {
    env::var_os(name).filter(|value| !value.is_empty())
}
