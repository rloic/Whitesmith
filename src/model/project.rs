use std::{io, fs};
use std::path::{Path};
use crate::model::versioning::Versioning;
use crate::model::job::{Job};
use crate::model::commands::{Commands};
use std::time::{Duration};
use std::fs::{File};
use std::io::{Write};
use serde::{Serialize, Deserialize};
use std::process::{Command, Stdio};
use colored::Colorize;
use threadpool::ThreadPool;
use crate::model::aliases::Aliases;
use crate::model::job::cmd_env::CmdEnv;
use crate::model::limits::Limits;
use crate::model::version::Version;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectVersionOnly {
    pub version: Version,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub version: Version,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, skip_serializing)]
    pub working_directory: String,
    #[serde(default, skip_serializing)]
    pub source_directory: String,
    #[serde(default, skip_serializing)]
    pub log_directory: String,
    #[serde(default, skip_serializing)]
    pub summary_file: String,
    pub versioning: Versioning,
    pub commands: Commands,
    pub experiments: Vec<Job>,
    #[serde(default, with = "humantime_serde", alias = "timeout")]
    pub global_timeout: Option<Duration>,
    #[serde(default = "default_nb_iterations")]
    pub iterations: u32,
    #[serde(default)]
    pub aliases: Aliases,
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub zip_with: Vec<String>,
    #[serde(default)]
    pub limits: Option<Limits>,
}

fn default_nb_iterations() -> u32 {
    1
}

impl Project {
    pub fn clean(&self) {
        if Path::new(&self.summary_file).exists() {
            fs::remove_file(&self.summary_file)
                .expect("Cannot remove summary_file");
        }
        if Path::new(&self.log_directory).exists() {
            fs::remove_dir_all(&self.log_directory)
                .expect("Fail to remove logs directory");
        }
        self.commands.run_clean(&self.source_directory, &self.aliases);
        self.init();
    }

    pub fn write_headers(&self, file: &mut File) -> io::Result<()> {
        let mut csv_writer = csv::Writer::from_writer(file);
        csv_writer.write_record(&["name", "status", "time", "iteration"])?;
        Ok(())
    }

    pub fn run(&self, pool: ThreadPool) {
        let summary_tsv = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.summary_file);

        if let Ok(mut summary_tsv) = summary_tsv {
            self.write_headers(&mut summary_tsv)
                .expect("Failed to wrap the headers of the summary file");
        }

        for experiment in &self.experiments {
            experiment.exec_on_pool(pool.clone(), self, &self.aliases);
        }
    }

    pub fn requires_overrides(&self) -> bool {
        let mut requires_overrides = false;
        for (key, value) in self.aliases.iter() {
            let value = value.to_string();
            if let Some('!') = value.chars().next() {
                eprintln!("The key {0} must be overridden by '{1}'. Use (--override {0}:'{1}').", key, &value[1..]);
                requires_overrides = true;
            }
        }

        requires_overrides
    }

    fn cmd_envs(&self) -> Vec<CmdEnv> {
        let mut project_experiments = Vec::new();
        for job in &self.experiments {
            job.enqueue(&mut project_experiments, self, &self.aliases);
        }
        project_experiments
    }

    pub fn unlock_failed(&self) {
        for experiment in &self.cmd_envs() {
            if experiment.is_locked() && experiment.has_err_tag() {
                eprintln!("Unlocking {}", experiment.name());
                fs::remove_dir_all(&experiment.log_dir())
                    .expect(&format!("Cannot remove the log directory for {}", experiment.name()));
            }
        }
    }

    pub fn unlock_timeout(&self) {
        for experiment in &self.cmd_envs() {
            if experiment.is_locked() && experiment.has_timeout_tag() {
                eprintln!("Unlocking {}", experiment.name());
                fs::remove_dir_all(&experiment.log_dir())
                    .expect(&format!("Cannot remove the log directory for {}", experiment.name()));
            }
        }
    }

    pub fn unlock_in_progress(&self) {
        for experiment in &self.cmd_envs() {
            if experiment.is_locked() && !experiment.has_done_tag() {
                eprintln!("Unlocking {}", experiment.name());
                fs::remove_dir_all(&experiment.log_dir())
                    .expect(&format!("Cannot remove the log directory for {}", experiment.name()));
            }
        }
    }

    pub fn init(&self) {
        let dir = Path::new(&self.working_directory);
        if !dir.exists() {
            fs::create_dir_all(dir).expect("Cannot create working directory");
        }

        let dir = Path::new(&self.source_directory);
        if !dir.exists() {
            fs::create_dir_all(dir).expect("Cannot create source directory");
        }
    }

    pub fn build(&self) {
        if !Path::new(&self.source_directory).exists() {
            panic!("The source folder doesn't exists. Try using the --git option to fetch the sources.");
        }
        self.commands.run_build(&self.source_directory, &self.aliases);
    }

    pub fn display_status(&self, filters: &Option<Vec<String>>) {
        println!("{:<40}\t{:<40}\t{:<40}", "Name", "Status", "Date");

        let mut nb_failures = 0;
        let mut nb_timeouts = 0;
        let mut nb_done = 0;
        let mut nb_running = 0;

        let cmd_envs = self.cmd_envs();
        for cmd_env in &cmd_envs {
            if cmd_env.match_any(filters) {
                let (status, date) = if cmd_env.is_locked() {
                    if cmd_env.has_err_tag() {
                        let creation_date = cmd_env.tag_creation_date(&CmdEnv::ERR_TAG);
                        nb_failures += 1;
                        ("Failed".red(), creation_date)
                    } else if cmd_env.has_timeout_tag() {
                        let creation_date = cmd_env.tag_creation_date(&CmdEnv::TIMEOUT_TAG);
                        nb_timeouts += 1;
                        ("Timeout".yellow(), creation_date)
                    } else if cmd_env.has_done_tag() {
                        let creation_date = cmd_env.tag_creation_date(&CmdEnv::DONE_TAG);
                        nb_done += 1;
                        ("Done".green(), creation_date)
                    } else {
                        let creation_date = cmd_env.tag_creation_date(&CmdEnv::LOCK_TAG);
                        nb_running += 1;
                        ("Running".blue(), creation_date)
                    }
                } else {
                    ("No started".black(), None)
                };
                let date_str = date.map(|it| it.format("%F %R").to_string()).unwrap_or(String::new());
                println!("{:<40}\t{:<40}\t{:<40}", cmd_env.name(), &status, &date_str);
            }
        }

        eprintln!("==========================");
        eprintln!("Summary: ");
        eprintln!("{:>8} {:>5}/{}", "Done", nb_done.to_string().green(), cmd_envs.len());
        eprintln!("{:>8} {:>5}/{}", "Running", nb_running.to_string().blue(), cmd_envs.len());
        eprintln!("{:>8} {:>5}/{}", "Timeout", nb_timeouts.to_string().yellow(), cmd_envs.len());
        eprintln!("{:>8} {:>5}/{}", "Failures", nb_failures.to_string().red(), cmd_envs.len());
    }

    pub fn fetch_sources(&self) {
        let folder = Path::new(&self.source_directory);
        if folder.exists() && folder.is_dir() && folder.read_dir().unwrap().count() != 0 {
            let mut response = String::new();
            loop {
                print!("The source directory is non empty. Would you erase it and fetch the sources again ? (y/N): ");
                let _ = io::stdout().flush();
                response.clear();
                io::stdin().read_line(&mut response).unwrap();
                let response = response.trim();
                if ["", "y", "Y", "n", "N"].contains(&response) { break; }
            }

            let response = response.trim();
            if response == "y" || response == "Y" {
                fs::remove_dir_all(&self.source_directory).expect("Cannot delete source directory");
                fs::create_dir_all(&self.source_directory).expect("Cannot create source directory");
            } else {
                return;
            }
        }

        if self.versioning.url.starts_with("file:") {
            copy_dir_all(&self.versioning.url["file:".len()..], &self.source_directory)
                .expect("Cannot copy the sources to the working directory");
        } else if self.versioning.url.starts_with("scp:") {
            Command::new("scp")
                .current_dir(&self.working_directory)
                .arg("-r")
                .arg(&self.versioning.url["scp:".len()..])
                .arg("src")
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .status()
                .expect("Cannot copy the sources using the scp command");
        } else {
            Command::new("git")
                .current_dir(&self.working_directory)
                .arg("clone")
                .arg(&self.versioning.url)
                .arg("src")
                .status()
                .expect("Cannot clone the remove git project");

            if let Some(commit) = &self.versioning.commit {
                Command::new("git")
                    .current_dir(&self.source_directory)
                    .arg("checkout")
                    .arg(&commit)
                    .status()
                    .expect("Cannot execute the git checkout command");
            }

            if self.versioning.sub_modules {
                Command::new("git")
                    .current_dir(&self.source_directory)
                    .args(&["submodule", "update", "--init"])
                    .status()
                    .expect("Cannot initialize the sub modules");
            }
        }
    }
}

fn copy_dir_all<PathSrc, PathDest>(source: PathSrc, destination: PathDest) -> io::Result<()>
    where PathSrc: AsRef<Path>, PathDest: AsRef<Path>
{
    fs::create_dir_all(&destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_all(entry.path(), destination.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), destination.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}