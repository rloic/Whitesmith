use std::path::Path;
use crate::model::project::Project;

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
        .and_then(|it| it.to_str())
        .unwrap_or("/");

    if parent == "" {
        String::from(".")
    } else {
        parent.to_owned()
    }
}

fn file_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|it| it.to_str())
        .unwrap()
        .to_owned()
}

pub fn working_directory(path: &Path) -> String {
    format!("{}/{}.d", parent_of(path), file_name(path))
}

pub fn source_directory(path: &Path) -> String {
    format!("{}/{}.d/src", parent_of(path), file_name(path))
}

pub fn log_directory(path: &Path) -> String {
    format!("{}/{}.d/logs", parent_of(path), file_name(path))
}

pub fn summary_file(path: &Path) -> String {
    format!("{0}/{1}.d/{1}.tsv", parent_of(path), file_name(path))
}

pub fn zip_file(path: &Path, p: &Project) -> String {
    let time = chrono::Local::now()
        .format("%Y-%m-%dT%H-%M")
        .to_string();
    if let Some(commit) = &p.versioning.commit {
        format!("{}/{}#{}@{}.zip", parent_of(path), file_name(path), &commit[0..8], time)
    } else {
        format!("{}/{}@{}.zip", parent_of(path), file_name(path), time)
    }
}
