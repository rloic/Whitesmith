use fs::OpenOptions;
use std::cmp::max;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use threadpool::ThreadPool;
use crate::ABORT;
use crate::model::aliases::Aliases;
use crate::model::project::Project;
use serde::{Serialize, Deserialize};
use crate::model::computation_result::ComputationResult;
use crate::model::job::cmd_env::CmdEnv;
use crate::model::output::{Iterations, OutputLine, Seconds};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Cmd {
    pub name: String,
    pub cmd: String,
}

impl Cmd {
    pub(crate) fn enqueue(&self, queue: &mut Vec<CmdEnv>, project: Project, aliases: Aliases) {
        queue.push(CmdEnv {
            cmd: self.clone(),
            project,
            aliases,
        })
    }

    pub(crate) fn exec_on_pool(&self, _pool: ThreadPool, project: Project, aliases: Aliases) {
        let cmd_env = CmdEnv { cmd: self.clone(), project, aliases, };

        let mut summary_file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(&cmd_env.summary_file())
            .expect("Cannot open summary file");

        if *ABORT.lock().unwrap() { return; }
        let exp_log_directory = cmd_env.log_dir();
        if cmd_env.try_lock() {
            for i in 1..=max(1, cmd_env.project.iterations) {
                eprintln!("Start {} {}/{} ", cmd_env.name(), i, cmd_env.project.iterations);
                let stderr_file = exp_log_directory.clone().join(format!("run_{}.stderr", i));
                let computation_result = cmd_env.run(&stderr_file);
                eprintln!("End {} {}/{}  {:?}", cmd_env.name(), i, cmd_env.project.iterations, computation_result);

                let mut csv_writer = csv::WriterBuilder::new()
                    .has_headers(false)
                    .from_writer(&mut summary_file);

                let (status, time) = match computation_result {
                    ComputationResult::Ok(duration) => ("Ok", Seconds(duration.as_secs_f64())),
                    ComputationResult::Timeout(duration) => ("Timeout", Seconds(duration.as_secs_f64())),
                    ComputationResult::Error(duration) => ("Error", Seconds(duration.as_secs_f64())),
                };

                let outline = OutputLine {
                    name: cmd_env.name(),
                    status: status.to_string(),
                    time,
                    iterations: Iterations(i, cmd_env.project.iterations)
                };

                csv_writer.serialize(outline)
                    .unwrap();

                if computation_result.is_err() {
                    cmd_env.add_err_tag();
                    if cmd_env.project.debug {
                        eprintln_file(&stderr_file);
                        return;
                    } else {
                        break;
                    }
                } else if computation_result.is_timeout() {
                    cmd_env.add_timeout_tag();
                }
            }
            cmd_env.add_done_tag();
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