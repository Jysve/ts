use std::{
    env,
    ffi::OsString,
    fs::{self, File},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    process,
};

use anyhow::{Context, Result, anyhow};

use crate::model::Store;

pub fn data_file() -> Result<PathBuf> {
    if let Some(path) = nonempty_env("TS_DATA_FILE") {
        return Ok(PathBuf::from(path));
    }

    if let Some(path) = nonempty_env("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(path).join("ts").join("todos.json"));
    }

    if let Some(home) = nonempty_env("HOME") {
        return Ok(PathBuf::from(home)
            .join(".config")
            .join("ts")
            .join("todos.json"));
    }

    Err(anyhow!(
        "Could not determine a config directory. Set TS_DATA_FILE to a writable file path."
    ))
}

pub fn load(path: &Path) -> Result<Store> {
    match fs::read(path) {
        Ok(bytes) => decode(path, &bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => load_legacy(path),
        Err(error) => Err(error).with_context(|| format!("Could not read {}", path.display())),
    }
}

fn load_legacy(path: &Path) -> Result<Store> {
    if nonempty_env("TS_DATA_FILE").is_some() {
        return Ok(Store::default());
    }

    let legacy_path = if let Some(data_home) = nonempty_env("XDG_DATA_HOME") {
        Some(PathBuf::from(data_home).join("ts").join("todos.json"))
    } else {
        nonempty_env("HOME").map(|home| {
            PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("ts")
                .join("todos.json")
        })
    };
    let Some(legacy_path) = legacy_path.filter(|legacy| legacy != path) else {
        return Ok(Store::default());
    };

    match fs::read(&legacy_path) {
        Ok(bytes) => {
            let store = decode(&legacy_path, &bytes)?;
            save(path, &store).with_context(|| {
                format!(
                    "Could not migrate todo data from {} to {}",
                    legacy_path.display(),
                    path.display()
                )
            })?;
            Ok(store)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Store::default()),
        Err(error) => {
            Err(error).with_context(|| format!("Could not read {}", legacy_path.display()))
        }
    }
}

fn decode(path: &Path, bytes: &[u8]) -> Result<Store> {
    let mut store: Store = serde_json::from_slice(bytes)
        .with_context(|| format!("Could not parse todo data in {}", path.display()))?;
    store.normalize_next_id();
    Ok(store)
}

pub fn save(path: &Path, store: &Store) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("Could not create {}", parent.display()))?;
    }

    let bytes = serde_json::to_vec_pretty(store).context("Could not encode todo data")?;
    let mut temporary_name: OsString = path.as_os_str().to_owned();
    temporary_name.push(format!(".tmp-{}", process::id()));
    let temporary_path = PathBuf::from(temporary_name);

    let result = (|| -> Result<()> {
        let mut file = File::create(&temporary_path)
            .with_context(|| format!("Could not create {}", temporary_path.display()))?;
        file.write_all(&bytes)
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
