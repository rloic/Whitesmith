use std::path::Path;

pub mod project;
pub mod versioning;
pub mod experiment;
pub mod commands;
pub mod computation;
pub mod outputs;

// Utils
fn parent_of(path: &Path) -> String {
    path.parent()
        .and_then(|it| it.to_str())
        .unwrap_or("/")
        .to_owned()
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

