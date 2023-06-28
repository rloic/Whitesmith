use std::collections::HashMap;
use crate::model::experiment::{Cmd};
use crate::model::project::{Project};
use std::path::PathBuf;
use std::fs;
use std::fs::OpenOptions;
use chrono::{Local, DateTime};
use crate::model::commands::restore_str;

pub struct Tag {
    pub name: &'static str,
}

pub struct ProjectExperiment<'e, 'p> {
    pub experiment: &'e Cmd,
    pub project: &'p Project,
    pub shortcuts: HashMap<String, String>
}

impl<'e, 'p> ProjectExperiment<'e, 'p> {
    pub(crate) const LOCK_TAG: Tag = Tag { name: "_lock" };
    pub(crate) const ERR_TAG: Tag = Tag { name: "_err" };
    pub(crate) const TIMEOUT_TAG: Tag = Tag { name: "_timeout" };
    pub(crate) const DONE_TAG: Tag = Tag { name: "_done" };

    pub fn name(&self) -> String {
        restore_str(&self.experiment.name, &self.shortcuts)
    }

    pub fn log_dir(&self) -> PathBuf {
        let dir = PathBuf::from(&self.project.log_directory)
            .join(&self.name());
        if !dir.exists() {
            fs::create_dir_all(&dir)
                .expect("Log dir already exists");
        }
        dir
    }

    pub fn tag_creation_date(&self, tag: &Tag) -> Option<DateTime<Local>> {
        let done_file = self.log_dir().join(tag.name);
        let creation_date = done_file.metadata()
            .and_then(|meta| meta.created())
            .ok();

        creation_date.map(|it| chrono::DateTime::from(it))
    }

    pub fn has_err_tag(&self) -> bool { self.has_tag(&ProjectExperiment::ERR_TAG) }

    pub fn has_timeout_tag(&self) -> bool { self.has_tag(&ProjectExperiment::TIMEOUT_TAG) }

    pub fn has_done_tag(&self) -> bool { self.has_tag(&ProjectExperiment::DONE_TAG) }

    pub fn is_locked(&self) -> bool {
        self.has_tag(&ProjectExperiment::LOCK_TAG)
    }

    pub fn add_err_tag(&self) {
        self.add_tag(&ProjectExperiment::ERR_TAG)
    }

    pub fn add_timeout_tag(&self) {
        self.add_tag(&ProjectExperiment::TIMEOUT_TAG)
    }

    pub fn add_done_tag(&self) {
        self.add_tag(&ProjectExperiment::DONE_TAG)
    }

    pub fn try_lock(&self) -> bool {
        let lock_file = self.log_dir().join(ProjectExperiment::LOCK_TAG.name);

        let creation = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_file);
        creation.is_ok()
    }

    pub fn match_any(&self, names: &Option<Vec<String>>) -> bool {
        if let Some(names) = names {
            names.iter().any(|it| it == &self.experiment.name || it == &self.name())
        } else {
            true
        }
    }

    fn has_tag(&self, tag: &Tag) -> bool {
        self.log_dir().join(tag.name).exists()
    }

    fn add_tag(&self, tag: &Tag) {
        let tag_file = self.log_dir().join(tag.name);

        OpenOptions::new()
            .write(true)
            .create(true)
            .open(tag_file)
            .expect(&format!("Cannot create {} file", tag.name));
    }
}