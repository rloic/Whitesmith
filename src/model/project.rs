use std::{io, fs};
use std::path::{Path, PathBuf};
use crate::model::versioning::Versioning;
use crate::model::experiment::{Experiment};
use crate::model::commands::Commands;
use std::time::{Duration};
use std::fs::{File, OpenOptions};
use std::io::{Write, BufReader, BufRead};
use std::cmp::{max};
use crate::model::outputs::Outputs;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use std::process::Command;
use colored::Colorize;
use chrono::Local;

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub working_directory: String,
    #[serde(default)]
    pub source_directory: String,
    #[serde(default)]
    pub log_directory: String,
    #[serde(default)]
    pub summary_file: String,
    pub versioning: Versioning,
    pub commands: Commands,
    pub experiments: Vec<Experiment>,
    #[serde(default)]
    pub outputs: Option<Outputs>,
    #[serde(default, with = "humantime_serde")]
    pub global_timeout: Option<Duration>,
    #[serde(default = "default_nb_iterations")]
    pub iterations: u32,
    #[serde(default)]
    pub shortcuts: HashMap<String, String>,
    #[serde(default)]
    pub debug: bool,
}

fn default_nb_iterations() -> u32 {
    1
}

impl Project {
    const LOCK_TAG: &'static str = "_lock";
    const ERR_TAG: &'static str = "_err";
    const TIMEOUT_TAG: &'static str = "_timeout";
    const DONE_TAG: &'static str = "_done";

    fn command_from(&self, cmd: &str, working_directory: &str) -> Command {
        let mut command = Command::new(cmd);
        command.current_dir(&working_directory);
        command
    }

    fn has_tag(&self, tag: &str, experiment: &Experiment) -> bool {
        self.log_dir(experiment).join(tag).exists()
    }

    fn tag(&self, tag: &str, experiment: &Experiment, uniq: bool) {
        let log_dir = self.log_dir(experiment);
        let tag_file = PathBuf::from(log_dir).join(tag);

        let mut open_options = OpenOptions::new();

        open_options.write(true);

        if uniq {
            open_options.create_new(true);
        } else {
            open_options.create(true);
        }

        open_options.open(tag_file)
            .expect(&format!("Cannot create {} file", tag));
    }

    fn has_timeout_tag(&self, e: &Experiment) -> bool {
        self.has_tag(Project::TIMEOUT_TAG, e)
    }

    fn add_timeout_tag(&self, e: &Experiment) {
        self.tag(Project::TIMEOUT_TAG, e, false);
    }

    fn has_done_tag(&self, e: &Experiment) -> bool {
        self.has_tag(Project::DONE_TAG, e)
    }

    fn add_done_tag(&self, e: &Experiment) {
        self.tag(Project::DONE_TAG, e, false);
    }

    fn has_err_tag(&self, e: &Experiment) -> bool {
        self.has_tag(Project::ERR_TAG, e)
    }

    fn add_err_tag(&self, e: &Experiment) {
        self.tag(Project::ERR_TAG, e, false);
    }

    fn is_locked(&self, experiment: &Experiment) -> bool {
        self.has_tag(Project::LOCK_TAG, experiment)
    }

    fn lock(&self, experiment: &Experiment) -> bool {
        let lock_file = self.log_dir(experiment).join(Project::LOCK_TAG);

        if let Err(_) = OpenOptions::new().write(true).create_new(true)
            .open(&lock_file) {
            true
        } else {
            false
        }
    }

    fn log_dir(&self, experiment: &Experiment) -> PathBuf {
        let dir = PathBuf::from(&self.log_directory).join(&experiment.name);
        if !dir.exists() {
            fs::create_dir_all(&dir)
                .expect("Log dir already exists");
        }
        dir
    }

    pub fn clean(&self) {
        if Path::new(&self.summary_file).exists() {
            fs::remove_file(&self.summary_file)
                .expect("Cannot remove summary_file");
        }
        if Path::new(&self.log_directory).exists() {
            fs::remove_dir_all(&self.log_directory)
                .expect("Fail to remove logs directory");
        }
        self.init();
    }

    pub fn write_headers(&self, file: &mut File) -> io::Result<()> {
        let mut scheme = String::new();
        scheme.push_str("name");

        if let Some(outputs) = &self.outputs {
            for column in &outputs.columns {
                if let Some(column) = column {
                    scheme.push('\t');
                    scheme.push_str(column);
                }
            }
        }

        scheme.push('\t');
        scheme.push_str("time");
        scheme.push('\t');
        scheme.push_str("iteration");
        scheme.push('\n');

        file.write_all(scheme.as_bytes())
    }

    pub fn run(&self) {
        let summary_tsv = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.summary_file);

        if let Ok(mut summary_tsv) = summary_tsv {
            self.write_headers(&mut summary_tsv)
                .expect("Failed to wrap the headers of the summary file");
        }

        let mut summary_tsv = fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open(&self.summary_file)
            .expect("Cannot open summary file");

        let mut open_mode = fs::OpenOptions::new();
        open_mode.create_new(true)
            .write(true)
            .append(true);

        for experiment in &self.experiments {
            let exp_log_directory = self.log_dir(experiment);
            if !self.lock(experiment) {
                for i in 0..max(1, self.iterations) {
                    println!("Run {} {}/{} ", experiment.name, i + 1, self.iterations);

                    let stdout_file = exp_log_directory.clone().join(format!("iteration_{}_stdout.txt", i));
                    let stderr_file = exp_log_directory.clone().join(format!("iteration_{}_stderr.txt", i));

                    let status = self.commands.run_exec(
                        &self.source_directory,
                        &self.shortcuts,
                        &experiment.parameters,
                        open_mode.open(&stdout_file).expect("Cannot create stdout file"),
                        open_mode.open(&stderr_file).expect("Cannot create stderr file"),
                        experiment.timeout.or(self.global_timeout),
                    );

                    let mut fields = Vec::new();

                    if status.is_ok() {
                        if let Some(outputs) = &self.outputs {
                            let log_file = File::open(&stdout_file)
                                .expect(&format!("Cannot open experiment `{}` log_file", experiment.name));
                            fields.extend(outputs.get_results(log_file));
                        }
                    } else {
                        if let Some(outputs) = &self.outputs {
                            for column in &outputs.columns {
                                if column.is_some() { fields.push("-".to_owned()); }
                            }
                        }
                    }

                    println!("  {:?}", status);

                    let mut tsv_line = String::new();
                    tsv_line.push_str(&experiment.name);
                    for field in &fields {
                        tsv_line.push('\t');
                        tsv_line.push_str(field);
                    }
                    tsv_line.push('\t');
                    tsv_line.push_str(&status.to_string());
                    tsv_line.push('\t');
                    tsv_line.push_str(&format!("{}/{}", i + 1, self.iterations));
                    tsv_line.push('\n');

                    summary_tsv.write_all(tsv_line.as_bytes())
                        .expect("Cannot write result into the summary file");

                    if status.is_err() {
                        self.add_err_tag(experiment);
                        if self.debug {
                            eprintln_file(&stderr_file);
                            return;
                        } else {
                            break;
                        }
                    } else if status.is_timeout() {
                        self.add_timeout_tag(experiment);
                    }
                }
                self.add_done_tag(&experiment);
            }
        }
    }

    pub fn requires_overrides(&self) -> bool {
        let mut requires_overrides = false;
        for (key, value) in self.shortcuts.iter() {
            if let Some('!') = value.chars().next() {
                eprintln!("The key {0} must be overridden by '{1}'. Use (--override {0}:'{1}').", key, &value[1..]);
                requires_overrides = true;
            }
        }

        requires_overrides
    }

    pub fn unlock_failed(&self) {
        for experiment in &self.experiments {
            if self.is_locked(experiment) && self.has_err_tag(experiment) {
                println!("Unlocking {}", experiment.name);
                fs::remove_dir_all(&self.log_dir(experiment))
                    .expect(&format!("Cannot remove the log directory for {}", experiment.name));
            }
        }
    }

    pub fn unlock_timeout(&self) {
        for experiment in &self.experiments {
            if self.is_locked(experiment) && self.has_timeout_tag(experiment) {
                println!("Unlocking {}", experiment.name);
                fs::remove_dir_all(&self.log_dir(experiment))
                    .expect(&format!("Cannot remove the log directory for {}", experiment.name));
            }
        }
    }

    pub fn unlock_killed(&self) {
        for experiment in &self.experiments {
            if self.is_locked(experiment) && !self.has_done_tag(experiment) {
                println!("Unlocking {}", experiment.name);
                fs::remove_dir_all(&self.log_dir(experiment))
                    .expect(&format!("Cannot remove the log directory for {}", experiment.name));
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
        self.commands.run_build(&self.source_directory, &self.shortcuts);
    }

    pub fn display_status(&self) {
        println!("{:<40}\t{}", "Name", "Status");
        for experiment in &self.experiments {
            let status = if self.is_locked(experiment) {
                if self.has_err_tag(experiment) {
                    "Failed".red()
                } else if self.has_timeout_tag(experiment) {
                    "Timeout".yellow()
                } else if self.has_done_tag(experiment) {
                    "Done".green()
                } else {
                    let lock_file = self.log_dir(experiment).join(Project::LOCK_TAG);
                    let creation_date = lock_file.metadata()
                        .and_then(|meta| meta.created())
                        .ok();

                    if let Some(creation_date) = creation_date {
                        let date: chrono::DateTime<Local> = chrono::DateTime::from(creation_date);
                        format!("{} since {}", "Running".blue(), date.format("%F %R").to_string()).black()
                    } else {
                        "Running".blue()
                    }
                }
            } else {
                "No started".black()
            };
            println!("{:<40}\t{}", experiment.name, &status);
        }
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

        self.command_from("git", &self.working_directory)
            .arg("clone")
            .arg(&self.versioning.url)
            .arg("src")
            .status()
            .expect("Cannot clone the remove git project");

        if let Some(commit) = &self.versioning.commit {
            self.command_from("git", &self.source_directory)
                .arg("checkout")
                .arg(&commit)
                .status()
                .expect("Cannot execute the git checkout command");
        }

        if self.versioning.sub_modules {
            self.command_from("git", &self.source_directory)
                .args(&["submodule", "update", "--init"])
                .status()
                .expect("Cannot initialize the sub modules");
        }
    }
}

fn eprintln_file(path: &PathBuf) {
    let file_buf = BufReader::new(File::open(path).expect(&format!("Cannot open `{:?}`", path)));
    eprintln!("```");
    for line in file_buf.lines() {
        let line = line.unwrap();
        eprintln!("{}", &line);
    }
    eprintln!("```");
}