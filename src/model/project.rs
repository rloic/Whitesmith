use std::{io, fs};
use std::path::{Path, PathBuf};
use crate::model::versioning::Versioning;
use crate::model::experiment::{Experiment};
use crate::model::commands::Commands;
use std::time::{Duration};
use std::fs::{File, OpenOptions};
use std::io::{Write, BufReader, BufRead};
use std::cmp::{max};
use crate::model::computation::ComputationResult;
use crate::model::outputs::Outputs;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use std::process::Command;

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
    pub outputs: Outputs,
    #[serde(default, with = "humantime_serde")]
    pub global_timeout: Option<Duration>,
    pub iterations: u32,
    #[serde(default)]
    pub shortcuts: HashMap<String, String>,
    #[serde(default)]
    pub debug: bool
}

impl Project {
    fn command_from(&self, cmd: &str, working_directory: &str) -> Command {
        let mut command = Command::new(cmd);
        command.current_dir(&working_directory);
        command
    }

    fn done(&self, experiment: &Experiment) {
        let log_dir = self.log_dir(experiment);
        let done_file = PathBuf::from(log_dir).join("_done");

        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&done_file)
            .expect("Cannot create done file");
    }

    fn is_locked(&self, experiment: &Experiment) -> bool {
        let lock_file = self.log_dir(experiment).join("_lock");

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

        for column in &self.outputs.columns {
            if let Some(column) = column {
                scheme.push('\t');
                scheme.push_str(column);
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
            if !self.is_locked(experiment) {
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

                    match status {
                        ComputationResult::Ok(_) => {
                            let log_file = File::open(&stdout_file)
                                .expect(&format!("Cannot open experiment `{}` log_file", experiment.name));
                            fields.extend((&self.outputs).get_results(log_file));
                        }
                        _ => {
                            for column in &self.outputs.columns {
                                if column.is_some() { fields.push("-".to_owned()); }
                            }
                        }
                    }
                    println!("\n  {:?}", status);

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

                    if let ComputationResult::Error = status {
                        if self.debug {
                            let err_buf = BufReader::new(File::open(&stderr_file).expect("Cannot open err file"));
                            eprintln!("```");
                            for line in err_buf.lines() {
                                let line = line.unwrap();
                                eprintln!("{}", &line);
                            }
                            eprintln!("```");
                            return;
                        } else {
                            break
                        }
                    }

                }
                self.done(&experiment);
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