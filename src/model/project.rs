use std::{io, fs};
use std::path::{Path, PathBuf};
use crate::model::versioning::Versioning;
use crate::model::experiment::{Experiment};
use crate::model::commands::Commands;
use std::time::{Duration};
use std::fs::{File};
use std::io::{Write, BufReader, BufRead};
use std::cmp::{max};
use crate::model::outputs::Outputs;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use std::process::Command;
use colored::Colorize;
use crate::model::project_experiment::ProjectExperiment;

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
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
    pub experiments: Vec<Experiment>,
    #[serde(default)]
    pub outputs: Option<Outputs>,
    #[serde(default, with = "humantime_serde", alias = "timeout")]
    pub global_timeout: Option<Duration>,
    #[serde(default = "default_nb_iterations")]
    pub iterations: u32,
    #[serde(default)]
    pub shortcuts: HashMap<String, String>,
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub zip_with: Vec<String>,
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
        self.commands.run_clean(&self.source_directory, &self.shortcuts);
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
        scheme.push_str("status");
        scheme.push('\t');
        scheme.push_str("time");
        scheme.push('\t');
        scheme.push_str("iteration");
        scheme.push('\n');

        file.write_all(scheme.as_bytes())
    }

    pub fn experiments(&self) -> impl Iterator<Item = ProjectExperiment> {
        self.experiments.iter()
            .map(move |it| ProjectExperiment { experiment: it, project: self })
    }

    pub fn run(&self, filters: &Option<Vec<String>>) {
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

        let mut experiments = self.experiments().collect::<Vec<_>>();
        experiments.sort_by_key(|e| e.experiment.difficulty);
        for experiment in experiments {
            if experiment.math_any(filters) {
                let exp_log_directory = experiment.log_dir();
                if experiment.try_lock() {
                    for i in 0..max(1, self.iterations) {
                        println!("Run {} {}/{} ", experiment.name(), i + 1, self.iterations);

                        let stdout_file = exp_log_directory.clone().join(format!("iteration_{}_stdout.txt", i));
                        let stderr_file = exp_log_directory.clone().join(format!("iteration_{}_stderr.txt", i));

                        let status = self.commands.run_exec(
                            &experiment.project.source_directory,
                            &experiment.project.shortcuts,
                            &experiment.experiment.parameters,
                            open_mode.open(&stdout_file).expect("Cannot create stdout file"),
                            open_mode.open(&stderr_file).expect("Cannot create stderr file"),
                            experiment.experiment.timeout.or(self.global_timeout),
                        );

                        let mut fields = Vec::new();

                        if status.is_ok() {
                            if let Some(outputs) = &self.outputs {
                                let log_file = File::open(&stdout_file)
                                    .expect(&format!("Cannot open experiment `{}` log_file", experiment.name()));
                                fields.extend(outputs.get_results(log_file));
                            }
                        } else {
                            if let Some(outputs) = &self.outputs {
                                for column in &outputs.columns {
                                    if column.is_some() { fields.push(String::from("-")); }
                                }
                            }
                        }

                        println!("  {:?}", status);

                        let mut tsv_line = String::new();
                        tsv_line.push_str(&experiment.name());
                        for field in &fields {
                            tsv_line.push('\t');
                            tsv_line.push_str(field);
                        }
                        tsv_line.push('\t');
                        tsv_line.push_str(&status.to_string());
                        tsv_line.push('\t');
                        tsv_line.push_str(&status.time_str());
                        tsv_line.push('\t');
                        tsv_line.push_str(&format!("{}/{}", i + 1, self.iterations));
                        tsv_line.push('\n');

                        summary_tsv.write_all(tsv_line.as_bytes())
                            .expect("Cannot write result into the summary file");

                        if status.is_err() {
                            experiment.add_err_tag();
                            if self.debug {
                                eprintln_file(&stderr_file);
                                return;
                            } else {
                                break;
                            }
                        } else if status.is_timeout() {
                            experiment.add_timeout_tag();
                        }
                    }
                    experiment.add_done_tag();
                }
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
        for experiment in self.experiments() {
            if experiment.is_locked() && experiment.has_err_tag() {
                println!("Unlocking {}", experiment.name());
                fs::remove_dir_all(&experiment.log_dir())
                    .expect(&format!("Cannot remove the log directory for {}", experiment.name()));
            }
        }
    }

    pub fn unlock_timeout(&self) {
        for experiment in self.experiments() {
            if experiment.is_locked() && experiment.has_timeout_tag() {
                println!("Unlocking {}", experiment.name());
                fs::remove_dir_all(&experiment.log_dir())
                    .expect(&format!("Cannot remove the log directory for {}", experiment.name()));
            }
        }
    }

    pub fn unlock_in_progress(&self) {
        for experiment in self.experiments() {
            if experiment.is_locked() && !experiment.has_done_tag() {
                println!("Unlocking {}", experiment.name());
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
        self.commands.run_build(&self.source_directory, &self.shortcuts);
    }

    pub fn display_status(&self, filters: &Option<Vec<String>>) {
        println!("{:<40}\t{:<40}\t{:<40}", "Name", "Status", "Date");
        let mut experiments = self.experiments().collect::<Vec<_>>();
        experiments.sort_by_key(|e| e.name());

        let mut nb_failures = 0;
        let mut nb_timeouts = 0;
        let mut nb_done = 0;
        let mut nb_running = 0;

        for experiment in &experiments {
            if experiment.math_any(filters) {
                let (status, date) = if experiment.is_locked() {
                    if experiment.has_err_tag() {
                        let creation_date = experiment.tag_creation_date(&ProjectExperiment::ERR_TAG);
                        nb_failures += 1;
                        ("Failed".red(), creation_date)
                    } else if experiment.has_timeout_tag() {
                        let creation_date = experiment.tag_creation_date(&ProjectExperiment::TIMEOUT_TAG);
                        nb_timeouts += 1;
                        ("Timeout".yellow(), creation_date)
                    } else if experiment.has_done_tag() {
                        let creation_date = experiment.tag_creation_date(&ProjectExperiment::DONE_TAG);
                        nb_done += 1;
                        ("Done".green(), creation_date)
                    } else {
                        let creation_date = experiment.tag_creation_date(&ProjectExperiment::LOCK_TAG);
                        nb_running += 1;
                        ("Running".blue(), creation_date)
                    }
                } else {
                    ("No started".black(), None)
                };
                let date_str = date.map(|it| it.format("%F %R").to_string()).unwrap_or(String::new());
                println!("{:<40}\t{:<40}\t{:<40}", experiment.name(), &status, &date_str);
            }
        }

        println!("==========================");
        println!("Summary: ");
        println!("{:>8} {:>5}/{}", "Done", nb_done.to_string().green(), experiments.len());
        println!("{:>8} {:>5}/{}", "Running", nb_running.to_string().blue(), experiments.len());
        println!("{:>8} {:>5}/{}", "Timeout", nb_timeouts.to_string().yellow(), experiments.len());
        println!("{:>8} {:>5}/{}", "Failures", nb_failures.to_string().red(), experiments.len());
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
            copy_dir_all(&self.versioning.url["file".len() + 1..], &self.source_directory)
                .expect("Cannot copy the sources to the working directory");
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

fn eprintln_file(path: &PathBuf) {
    let file_buf = BufReader::new(File::open(path)
        .expect(&format!("Cannot open `{:?}`", path)));
    eprintln!("```");
    for line in file_buf.lines() {
        let line = line.unwrap();
        eprintln!("{}", &line);
    }
    eprintln!("```");
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