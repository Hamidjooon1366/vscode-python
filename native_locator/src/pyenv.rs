// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use regex::Regex;
use std::fs;
use std::path::PathBuf;

use crate::known;
use crate::messaging;
use crate::utils::find_python_binary_path;

#[cfg(windows)]
fn get_home_pyenv_dir(environment: &impl known::Environment) -> Option<String> {
    let home = environment.get_user_home()?;
    PathBuf::from(home)
        .join(".pyenv")
        .join("pyenv-win")
        .into_os_string()
        .into_string()
        .ok()
}

#[cfg(unix)]
fn get_home_pyenv_dir(environment: &impl known::Environment) -> Option<String> {
    let home = environment.get_user_home()?;
    PathBuf::from(home)
        .join(".pyenv")
        .into_os_string()
        .into_string()
        .ok()
}

fn get_binary_from_known_paths(environment: &impl known::Environment) -> Option<String> {
    for known_path in environment.get_know_global_search_locations() {
        let bin = known_path.join("pyenv");
        if bin.exists() {
            return bin.into_os_string().into_string().ok();
        }
    }
    None
}

fn get_pyenv_dir(environment: &impl known::Environment) -> Option<String> {
    // Check if the pyenv environment variables exist: PYENV on Windows, PYENV_ROOT on Unix.
    // They contain the path to pyenv's installation folder.
    // If they don't exist, use the default path: ~/.pyenv/pyenv-win on Windows, ~/.pyenv on Unix.
    // If the interpreter path starts with the path to the pyenv folder, then it is a pyenv environment.
    // See https://github.com/pyenv/pyenv#locating-the-python-installation for general usage,
    // And https://github.com/pyenv-win/pyenv-win for Windows specifics.

    match environment.get_env_var("PYENV_ROOT".to_string()) {
        Some(dir) => Some(dir),
        None => match environment.get_env_var("PYENV".to_string()) {
            Some(dir) => Some(dir),
            None => get_home_pyenv_dir(environment),
        },
    }
}

fn get_pyenv_binary(environment: &impl known::Environment) -> Option<String> {
    let dir = get_pyenv_dir(environment)?;
    let exe = PathBuf::from(dir).join("bin").join("pyenv");
    if fs::metadata(&exe).is_ok() {
        exe.into_os_string().into_string().ok()
    } else {
        get_binary_from_known_paths(environment)
    }
}

fn get_pyenv_version(folder_name: String) -> Option<String> {
    // Stable Versions = like 3.10.10
    let python_regex = Regex::new(r"^(\d+\.\d+\.\d+)$").unwrap();
    match python_regex.captures(&folder_name) {
        Some(captures) => match captures.get(1) {
            Some(version) => Some(version.as_str().to_string()),
            None => None,
        },
        None => {
            // Dev Versions = like 3.10-dev
            let python_regex = Regex::new(r"^(\d+\.\d+-dev)$").unwrap();
            match python_regex.captures(&folder_name) {
                Some(captures) => match captures.get(1) {
                    Some(version) => Some(version.as_str().to_string()),
                    None => None,
                },
                None => {
                    // Alpha Versions = like 3.10.0a3
                    let python_regex = Regex::new(r"^(\d+\.\d+.\d+a\d+)").unwrap();
                    match python_regex.captures(&folder_name) {
                        Some(captures) => match captures.get(1) {
                            Some(version) => Some(version.as_str().to_string()),
                            None => None,
                        },
                        None => None,
                    }
                }
            }
        }
    }
}

pub fn find_and_report(
    dispatcher: &mut impl messaging::MessageDispatcher,
    environment: &impl known::Environment,
) -> Option<()> {
    let pyenv_dir = get_pyenv_dir(environment)?;

    if let Some(pyenv_binary) = get_pyenv_binary(environment) {
        dispatcher.report_environment_manager(messaging::EnvManager::new(vec![pyenv_binary], None));
    }

    let versions_dir = PathBuf::from(&pyenv_dir)
        .join("versions")
        .into_os_string()
        .into_string()
        .ok()?;

    let pyenv_binary_for_activation = match get_pyenv_binary(environment) {
        Some(binary) => binary,
        None => "pyenv".to_string(),
    };
    for entry in fs::read_dir(&versions_dir).ok()? {
        if let Ok(path) = entry {
            let path = path.path();
            if path.is_dir() {
                if let Some(executable) = find_python_binary_path(&path) {
                    let version =
                        get_pyenv_version(path.file_name().unwrap().to_string_lossy().to_string());

                    // If we cannot extract version, this isn't a valid pyenv environment.
                    // Or its one that we're not interested in.
                    if version.is_none() {
                        continue;
                    }
                    let env_path = path.to_string_lossy().to_string();
                    let activated_run = match version.clone() {
                        Some(version) => Some(vec![
                            pyenv_binary_for_activation.clone(),
                            "local".to_string(),
                            version.clone(),
                        ]),
                        None => None,
                    };
                    dispatcher.report_environment(messaging::PythonEnvironment::new(
                        "Python".to_string(),
                        vec![executable.into_os_string().into_string().unwrap()],
                        messaging::PythonEnvironmentCategory::Pyenv,
                        version,
                        activated_run,
                        Some(env_path.clone()),
                        Some(env_path),
                    ));
                }
            }
        }
    }

    None
}