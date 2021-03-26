use std::{io, fs};
use std::path::Path;
use crate::model::versioning::Versioning;
use crate::model::experiment::{Experiment};
use crate::model::commands::Commands;
use std::time::{Duration};
use std::fs::File;
use std::io::{Write};
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
}

impl Project {
    fn command_from(&self, cmd: &str, working_directory: &str) -> Command {
        let mut command = Command::new(cmd);
        command.current_dir(&working_directory);
        command
    }

    fn is_locked(&self, experiment: &Experiment) -> io::Result<bool> {
        let log_dir = self.log_dir(experiment)?;
        let lock_file = &format!("{}/_lock", log_dir);
        let lock_file = Path::new(lock_file);
        if lock_file.exists() && lock_file.is_file() {
            Ok(true)
        } else {
            fs::File::create(lock_file)?;
            Ok(false)
        }
    }

    fn log_dir(&self, experiment: &Experiment) -> io::Result<String> {
        let path = format!("{}/{}", self.log_directory, experiment.name);
        let dir = Path::new(&path);
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }
        Ok(path)
    }

    pub fn clean(&self) -> io::Result<()> {
        if Path::new(&self.summary_file).exists() {
            fs::remove_file(&self.summary_file)?;
        }
        fs::remove_dir_all(&self.log_directory)?;
        fs::create_dir_all(&self.log_directory)?;
        Ok(())
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
        scheme.push('\n');

        file.write_all(scheme.as_bytes())
    }

    pub fn run(&self) -> io::Result<()> {
        let already_exists = Path::new(&self.summary_file).exists();

        let mut summary_tsv = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(&self.summary_file)?;

        if !already_exists {
            self.write_headers(&mut summary_tsv)?;
        }

        let mut open_mode = fs::OpenOptions::new();
        open_mode.create_new(true)
            .write(true)
            .append(true);

        for experiment in &self.experiments {
            let exp_log_directory = self.log_dir(experiment)?;
            if !self.is_locked(experiment)? {
                for i in 0..max(1, self.iterations) {
                    println!("Run {} {}/{} ", experiment.name, i + 1, self.iterations);

                    let log_file = format!("{}/run_{}_log.txt", exp_log_directory, i);
                    let err_file = format!("{}/run_{}_err.txt", exp_log_directory, i);

                    let status = self.commands.run_exec(
                        &self.source_directory,
                        &self.shortcuts,
                        &experiment.parameters,
                        open_mode.open(&log_file)?,
                        open_mode.open(&err_file)?,
                        experiment.timeout.or(self.global_timeout),
                    );

                    let mut fields = Vec::new();

                    match status {
                        ComputationResult::Ok(_) => {
                            let log_file = File::open(&log_file)?;
                            fields.extend((&self.outputs).get_results(log_file)?);
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
                    tsv_line.push('\n');

                    summary_tsv.write_all(tsv_line.as_bytes())?;

                    if let ComputationResult::Error = status {
                        break
                    }

                }
            }
        }
        Ok(())
    }

    pub fn init(&self) -> io::Result<()> {
        let dir = Path::new(&self.working_directory);
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }

        let dir = Path::new(&self.source_directory);
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn build(&self) -> io::Result<()> {
        if !Path::new(&self.source_directory).exists() {
            println!("The source folder doesn't exists. Try using the --git option to fetch the sources.");
            return Ok(());
        }
        self.commands.run_build(&self.source_directory, &self.shortcuts);
        Ok(())
    }

    pub fn fetch_sources(&self) -> io::Result<()> {
        if Path::new(&self.source_directory).exists() {
            let mut response = String::new();
            loop {
                print!("The source directory is non empty. Would you erase it and fetch the sources again ? (y/N): ");
                io::stdout().flush()?;
                response.clear();
                io::stdin().read_line(&mut response)?;
                let response = response.trim();
                if ["", "y", "Y", "n", "N"].contains(&response) { break; }
            }

            let response = response.trim();
            if response == "y" || response == "Y" {
                fs::remove_dir_all(&self.source_directory)?;
                fs::create_dir_all(&self.source_directory)?;
            } else {
                return Ok(());
            }
        }

        self.command_from("git", &self.working_directory)
            .arg("clone")
            .arg(&self.versioning.url)
            .arg("src")
            .status()?;

        if let Some(commit) = &self.versioning.commit {
            self.command_from("git", &self.source_directory)
                .arg("checkout")
                .arg(&commit)
                .status()?;
        }

        if self.versioning.sub_modules {
            self.command_from("git", &self.source_directory)
                .args(&["submodule", "update", "--init"])
                .status()?;
        }

        Ok(())
    }
}