use crate::model::project::{Project};
use std::path::PathBuf;
use std::fs;
use std::fs::{OpenOptions};
use chrono::{Local, DateTime};
use crate::model::aliases::Aliases;
use crate::model::commands::restore_str;
use crate::model::computation_result::ComputationResult;
use crate::model::job::cmd::Cmd;

pub struct Tag {
    pub name: &'static str,
}

pub struct CmdEnv {
    pub cmd: Cmd,
    pub project: Project,
    pub aliases: Aliases
}

impl CmdEnv {
    pub(crate) const LOCK_TAG: Tag = Tag { name: "_lock" };
    pub(crate) const ERR_TAG: Tag = Tag { name: "_err" };
    pub(crate) const TIMEOUT_TAG: Tag = Tag { name: "_timeout" };
    pub(crate) const DONE_TAG: Tag = Tag { name: "_done" };

    pub fn name(&self) -> String {
        restore_str(&self.cmd.name, &self.aliases)
    }

    pub fn summary_file(&self) -> &String {
        &self.project.summary_file
    }

    pub fn run(&self, stderr_file: &PathBuf) -> ComputationResult {
        let mut open_mode = OpenOptions::new();
        open_mode.create_new(true)
            .write(true)
            .append(true);

        self.project.commands.run_exec(
            &self.project.source_directory,
            &self.aliases,
            &self.cmd.cmd,
            open_mode.open(stderr_file).expect("Cannot create stderr file"),
            self.project.global_timeout,
        )
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

    pub fn has_err_tag(&self) -> bool { self.has_tag(&CmdEnv::ERR_TAG) }

    pub fn has_timeout_tag(&self) -> bool { self.has_tag(&CmdEnv::TIMEOUT_TAG) }

    pub fn has_done_tag(&self) -> bool { self.has_tag(&CmdEnv::DONE_TAG) }

    pub fn is_locked(&self) -> bool {
        self.has_tag(&CmdEnv::LOCK_TAG)
    }

    pub fn add_err_tag(&self) {
        self.add_tag(&CmdEnv::ERR_TAG)
    }

    pub fn add_timeout_tag(&self) {
        self.add_tag(&CmdEnv::TIMEOUT_TAG)
    }

    pub fn add_done_tag(&self) {
        self.add_tag(&CmdEnv::DONE_TAG)
    }

    pub fn try_lock(&self) -> bool {
        let lock_file = self.log_dir().join(CmdEnv::LOCK_TAG.name);

        let creation = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_file);
        creation.is_ok()
    }

    pub fn match_any(&self, names: &Option<Vec<String>>) -> bool {
        if let Some(names) = names {
            names.iter().any(|it| it == &self.cmd.name || it == &self.name())
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