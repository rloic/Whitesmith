use std::path::{Path, PathBuf};
use crate::model::project::Project;
use std::ffi::OsStr;
use crate::model::versioning::Versioning;

pub mod project;
pub mod versioning;
pub mod experiment;
pub mod commands;
pub mod computation;
pub mod outputs;
pub mod project_experiment;

// Utils
fn parent_of(path: &Path) -> String {
    let parent = path.parent()
        .and_then(Path::to_str)
        .unwrap_or("/");

    if parent == "" {
        String::from(".")
    } else {
        parent.to_owned()
    }
}

fn file_name(path: &Path) -> String {
    path.file_stem()
        .and_then(OsStr::to_str)
        .unwrap()
        .to_owned()
}

pub fn working_directory(path: &PathBuf, versioning: &Versioning) -> String {
    let commit_hash = versioning.commit.as_ref()
        .map(|it| String::from(&it[..6]))
        .unwrap_or(String::new());
    format!("{}/{}{}.d", parent_of(path), file_name(path), commit_hash)
}

pub fn source_directory(path: &PathBuf, versioning: &Versioning) -> String {
    let commit_hash = versioning.commit.as_ref()
        .map(|it| String::from(&it[..6]))
        .unwrap_or(String::new());
    format!("{}/{}{}.d/src", parent_of(path), file_name(path), commit_hash)
}

pub fn log_directory(path: &PathBuf, versioning: &Versioning) -> String {
    let commit_hash = versioning.commit.as_ref()
        .map(|it| String::from(&it[..6]))
        .unwrap_or(String::new());
    format!("{}/{}{}.d/logs", parent_of(path), file_name(path), commit_hash)
}

pub fn summary_file(path: &PathBuf, versioning: &Versioning, is_zip_archive: bool) -> String {
    if is_zip_archive {
        let mut name = file_name(path);

        if let Some(pos) = name.find('#') {
            name = String::from(&name[..pos]) + ".tsv"
        }

        name
    } else {
        let commit_hash = versioning.commit.as_ref()
        .map(|it| String::from(&it[..6]))
        .unwrap_or(String::new());
        format!("{0}/{1}{2}.d/{1}.tsv", parent_of(path), file_name(path), commit_hash)
    }
}

pub fn zip_file(path: &PathBuf, p: &Project) -> String {
    let time = chrono::Local::now()
        .format("%Y-%m-%dT%H-%M")
        .to_string();
    if let Some(commit) = &p.versioning.commit {
        format!("{}/{}#{}@{}.zip", parent_of(path), file_name(path), &commit[0..8], time)
    } else {
        format!("{}/{}@{}.zip", parent_of(path), file_name(path), time)
    }
}
