use std::process::{Command, Stdio};
use std::collections::HashMap;
use std::fs::File;
use std::time::{Duration, Instant};
use crate::model::computation::ComputationResult;
use wait_timeout::ChildExt;
use serde::{Serialize, Deserialize};
use std::io;
use std::fmt::{Debug, Formatter};
use std::io::Write;

#[derive(Debug, Serialize, Deserialize)]
pub struct Commands {
    pub build: String,
    pub execute: String,
}

impl Commands {
    fn generate_build(&self, shortcuts: &HashMap<String, String>) -> BuildCommand {
        BuildCommand { sub_command: generate_command(&self.build, shortcuts) }
    }

    fn generate_executable(&self, shortcuts: &HashMap<String, String>, parameters: &Vec<String>) -> ExecutableCommand {
        let mut execute_with_parameters = self.execute.to_owned();
        for parameter in parameters {
            execute_with_parameters.push(' ');
            execute_with_parameters.push_str(parameter);
        }
        ExecutableCommand { sub_command: generate_command(&execute_with_parameters, shortcuts) }
    }

    pub fn run_build(&self, working_directory: &str, shortcuts: &HashMap<String, String>) {
        let build_command = self.generate_build(shortcuts);
        println!("Building project: ");
        println!("$ {:?}", &build_command.sub_command);
        if !build_command.run(working_directory){
            panic!("Cannot execute {:?}", build_command.sub_command);
        }
    }

    pub fn run_exec(
        &self,
        working_directory: &str,
        shortcuts: &HashMap<String, String>,
        parameters: &Vec<String>,
        log_file: File,
        err_file: File,
        timeout: Option<Duration>,
    ) -> ComputationResult {
        let executable_command = self.generate_executable(shortcuts, parameters);
        print!("$ {:?}", &executable_command.sub_command);
        let _ = io::stdout().flush();

        if let Some(timeout) = timeout {
            executable_command.run_with_timeout(working_directory, log_file, err_file, timeout)
        } else {
            executable_command.run(working_directory,log_file, err_file)
        }
    }
}

struct SubCommand {
    executable: String,
    args: Vec<String>
}

impl Debug for SubCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut content = String::from(&self.executable);
        content.push(' ');
        let mut curr_len = self.executable.len() + 1;

        for (i, element) in self.args.iter().enumerate() {
            content.push_str(element);
            curr_len += element.len();
            if i + 1 < self.args.len() && curr_len + self.args[i + 1].len() > 80 {
                content.push_str(" \\\n   > ");
                curr_len = 0;
            } else if i + 1 < self.args.len() {
                content.push(' ');
                curr_len += 1;
            }
        }

        f.write_str(&content)
    }
}

struct BuildCommand {
    sub_command: SubCommand
}

impl BuildCommand {
    fn run(&self, working_directory: &str) -> bool {
        Command::new(&self.sub_command.executable)
            .current_dir(working_directory)
            .args(&self.sub_command.args)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

struct ExecutableCommand {
    sub_command: SubCommand
}

impl ExecutableCommand {
    fn run(&self, working_directory: &str, log_file: File, err_file: File) -> ComputationResult {
        let clock = Instant::now();
        let success = Command::new(&self.sub_command.executable)
            .current_dir(working_directory)
            .args(&self.sub_command.args)
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(err_file))
            .status()
            .map(|status| status.success());

        if let Ok(success) = success {
            if success {
                ComputationResult::Ok(clock.elapsed())
            } else {
                ComputationResult::Error
            }
        } else {
            println!();
            panic!("The script cannot execute {:?}", self.sub_command);
        }
    }

    fn run_with_timeout(&self, working_directory: &str, log_file: File, err_file: File, timeout: Duration) -> ComputationResult {
        let clock = Instant::now();
        let success = Command::new(&self.sub_command.executable)
            .current_dir(working_directory)
            .args(&self.sub_command.args)
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(err_file))
            .spawn()
            .and_then(|mut child| {
                let res = child.wait_timeout(timeout);
                let _ = child.kill(); let _ = child.wait();
                res
            })
            .map(|opt_status| opt_status.map(|status| status.success()));

        if let Ok(success) = success {
            if let Some(success) = success {
                if success {
                    ComputationResult::Ok(clock.elapsed())
                } else {
                    ComputationResult::Error
                }
            } else {
                ComputationResult::Timeout(timeout)
            }
        } else {
            println!();
            panic!("The script cannot execute {:?}", self.sub_command);
        }
    }
}

fn generate_command(command_line: &str, shortcuts: &HashMap<String, String>) -> SubCommand {
    let mut old = command_line.to_owned();
    loop {
        let mut new = old.to_owned();
        for (key, value) in shortcuts.iter() {
            new = new.replace(&format!("{{{}}}", key), value);
        }
        if old == new {
            break;
        } else {
            old = new;
        }
    }
    let splitted = old.split(' ').collect::<Vec<_>>();
    let (&executable, args) = splitted.split_first().unwrap();
    let executable = executable.to_owned();
    let args = args.iter().map(|&it| it.to_owned()).collect::<Vec<_>>();
    SubCommand { executable, args }
}