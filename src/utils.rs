use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use crate::DATA_DIR;

// Case-insensitive path search from data_dir
pub fn get_path(path: &Path) -> Option<PathBuf> {
    let mut matched = DATA_DIR.to_owned();
    for elem in path.components() {
        let Ok(iter) = fs::read_dir(&matched) else {
            return None;
        };
        let mut found = String::new();
        for file in iter {
            let Ok(file) = file else { continue };
            let file_name = file.file_name();
            let file_name = file_name.to_string_lossy();
            if file_name.to_ascii_lowercase().as_str()
                == elem.as_os_str().to_ascii_lowercase().as_os_str()
            {
                found = file_name.to_string();
                break;
            }
        }
        if found.is_empty() {
            return None;
        }
        matched = matched.join(found);
    }
    Some(matched)
}

// We need this to deal with windows paths on non-windows platforms
pub fn to_path(input: &str) -> PathBuf {
    PathBuf::from(input.replace('\\', "/"))
}
