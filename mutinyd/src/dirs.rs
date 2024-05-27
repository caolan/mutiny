use std::os::unix::fs::PermissionsExt;
use std::env::{var, var_os, consts::OS};
use std::path::PathBuf;
use std::error::Error;
use std::fs;

pub fn user_runtime_dir_path() -> Result<PathBuf, std::env::VarError> {
    if OS == "macos" {
        // Just pick something sensible
        Ok(PathBuf::from(var("HOME")?).join("Library/Caches/TemporaryItems"))
    } else {
        // Assume following freedesktop.org specification (Linux etc)
        Ok(PathBuf::from(var("XDG_RUNTIME_DIR")?))
    }
}

pub fn user_data_dir_path() -> Result<PathBuf, std::env::VarError> {
    if OS == "macos" {
        // Just pick something sensible
        Ok(PathBuf::from(var("HOME")?).join("Library"))
    } else {
        // Assume following freedesktop.org specification (Linux etc)
        Ok(match var_os("XDG_DATA_HOME") {
            Some(val) => PathBuf::from(val),
            None => PathBuf::from(var("HOME")?).join(".local/share"),
        })
    }
}

fn open_private_app_dir(base: PathBuf) -> Result<PathBuf, Box<dyn Error>> {
    let p = base.join("mutiny");
    // Ensure path exists
    fs::create_dir_all(&p)?;
    // Restrict to current user
    fs::set_permissions(&p, fs::Permissions::from_mode(0o700))?;
    Ok(p)
}

pub fn open_app_data_dir() -> Result<PathBuf, Box<dyn Error>> {
    return open_private_app_dir(user_data_dir_path()?);
}

pub fn open_app_runtime_dir() -> Result<PathBuf, Box<dyn Error>> {
    return open_private_app_dir(user_runtime_dir_path()?);
}

