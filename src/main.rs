mod model;
mod tools;

use std::{thread};
use std::fs::File;
use std::io::{BufReader};
use std::path::{Path};

use crate::model::project::Project;
use clap::{App, Arg};
use crate::model::{working_directory, source_directory, log_directory, summary_file, zip_file};
use std::sync::Arc;
use crate::tools::RecursiveZipWriter;
use zip::CompressionMethod;

extern crate wait_timeout;
extern crate serde;
extern crate ron;
extern crate humantime;

const CONFIG_ARG: &str = "CONFIG";
const RUN_FLAG: &str = "run";
const BUILD_FLAG: &str = "build";
const CLEAN_FLAG: &str = "clean";
const WITH_KILLED_FLAG: &str = "with-killed";
const WITH_EXPIRED_FLAG: &str = "with-expired";
const WITH_FAILURE_FLAG: &str = "with-failed";
const GIT_FLAG: &str = "git";
const OVERRIDE_ARGS: &str = "override";
const DEBUG_FLAG: &str = "debug";
const NB_THREADS_ARG: &str = "nb_threads";
const GLOBAL_TIMEOUT_ARG: &str = "global_timeout";
const ZIP_FLAG: &str = "zip";

fn check_nb_thread(v: String) -> Result<(), String> {
    if let Ok(number) = v.parse::<usize>() {
        if number < 1 {
            Err("The number of threads must be strictly positive".to_owned())
        } else {
            Ok(())
        }
    } else {
        Err(format!("Cannot parse {} as usize", v))
    }
}

fn check_global_timeout(v: String) -> Result<(), String> {
    if let Ok(_) = v.parse::<humantime::Duration>() {
        Ok(())
    } else {
        Err(format!("Cannot parse {} as Duration", v))
    }
}

fn required_single_argument(name: &str) -> Arg {
    optional_single_argument(name)
        .required(true)
}

fn optional_single_argument(name: &str) -> Arg {
    Arg::with_name(name)
        .takes_value(true)
        .multiple(false)
}

fn optional_multiple_arguments(name: &str) -> Arg {
    Arg::with_name(name)
        .takes_value(true)
        .multiple(true)
}

fn flag(name: &str) -> Arg {
    Arg::with_name(name)
        .takes_value(false)
}

fn main() {
    let matches = App::new("whitesmith")
        .version("0.1")
        .author("Loïc Rouquette <loic.rouquette@insa-lyon.fr>")
        .arg(required_single_argument(CONFIG_ARG)
            .help("Configuration file")
            .index(1))
        .arg(flag(RUN_FLAG)
            .long(RUN_FLAG)
            .short("r")
            .help("Run the experiments. By default, the script only runs the experiment that were not already executed. To re-run all the experiments use the option --clean. To add some specific experiments see the --with-* flag descriptions"))
        .arg(flag(GIT_FLAG)
            .long(GIT_FLAG)
            .short("g")
            .help("Fetch sources from the git repository"))
        .arg(flag(BUILD_FLAG)
            .long(BUILD_FLAG)
            .short("b")
            .help("Build the project from sources (must be downloaded before)"))
        .arg(flag(CLEAN_FLAG)
            .long(CLEAN_FLAG)
            .help("Remove previous experiments results"))
        .arg(optional_multiple_arguments(OVERRIDE_ARGS)
            .long(OVERRIDE_ARGS)
            .help("Override the configuration shortcuts with custom value (usage: --override key:value)")
        )
        .arg(flag(DEBUG_FLAG)
            .long(DEBUG_FLAG)
            .short("d")
            .help("Run the experiments in debug mode, i.e. exit the executions on the first failure")
        )
        .arg(optional_single_argument(NB_THREADS_ARG)
            .long(NB_THREADS_ARG)
            .help("Set the number of parallel threads (default=1)")
            .validator(check_nb_thread)
        )
        .arg(optional_single_argument(GLOBAL_TIMEOUT_ARG)
            .long(GLOBAL_TIMEOUT_ARG)
            .short("T")
            .help("Override (or set) the global timeout")
            .validator(check_global_timeout)
        )
        .arg(flag(WITH_KILLED_FLAG)
            .long(WITH_KILLED_FLAG)
            .help("Allows to re-run the experiments that weren't finished in the previous call")
        )
        .arg(flag(WITH_EXPIRED_FLAG)
            .long(WITH_EXPIRED_FLAG)
            .help("Allows to re-run the experiments that reach the timeout in the previous call")
        )
        .arg(flag(WITH_FAILURE_FLAG)
            .long(WITH_FAILURE_FLAG)
            .help("Allows to re-run the experiments that failed in the previous call")
        )
        .arg(flag(ZIP_FLAG)
            .long(ZIP_FLAG)
            .help("Zip the logs into an archive at the end of the computation")
        )
        .get_matches();

    let path = matches.value_of("CONFIG").unwrap();
    let path = Path::new(path);
    let config_file = File::open(path)
        .expect("Cannot open the configuration file. Maybe the file doesn't exists or the permissions are too restrictive.");
    let mut project = ron::de::from_reader::<_, Project>(BufReader::new(config_file))
        .map_err(|e| e.to_string())
        .expect("Cannot parse the configuration file");

    project.working_directory = working_directory(path);
    project.source_directory = source_directory(path);
    project.log_directory = log_directory(path);
    project.summary_file = summary_file(path);
    project.debug = matches.is_present(DEBUG_FLAG);

    let zip_path = zip_file(path, &project);

    if let Some(values) = matches.values_of(OVERRIDE_ARGS) {
        for value in values {
            let fields = value.split(':').collect::<Vec<_>>();
            let (key, value) = (fields[0], fields[1]);
            project.shortcuts.insert(key.to_owned(), value.to_owned());
        }
    }

    if let Some(str_duration) = matches.value_of(GLOBAL_TIMEOUT_ARG) {
        project.global_timeout = Some(*str_duration.parse::<humantime::Duration>().unwrap());
    }

    let project = Arc::new(project);
    project.init();

    if matches.is_present(CLEAN_FLAG) {
        project.clean();
    }

    if matches.is_present(GIT_FLAG) {
        project.fetch_sources();
    }

    if matches.is_present(BUILD_FLAG) {
        project.build();
    }

    if matches.is_present(RUN_FLAG) {
        if project.requires_overrides() {
            return;
        }

        if matches.is_present(WITH_KILLED_FLAG) {
            project.unlock_killed();
        }

        if matches.is_present(WITH_EXPIRED_FLAG) {
            project.unlock_timeout();
        }

        if matches.is_present(WITH_FAILURE_FLAG) {
            project.unlock_failed();
        }

        if let Some(nb_threads) = matches.value_of(NB_THREADS_ARG) {
            let nb_threads = nb_threads.parse::<usize>().unwrap();
            let mut handlers = Vec::with_capacity(nb_threads);
            for _ in 0..nb_threads {
                let project = project.clone();
                handlers.push(thread::spawn(move || { project.run() }));
            }
            for handler in handlers { handler.join().unwrap(); }
        } else {
            project.run();
        }
    }

    if matches.is_present(ZIP_FLAG) {
        let zip_file = File::create(zip_path)
            .expect("Cannot create a the zip file");
        let mut archive = RecursiveZipWriter::new(zip_file)
            .compression_method(CompressionMethod::Stored);

        archive.add_path(Path::new(&project.log_directory))
            .expect("Fail to add the log directory to the zip archive");
        archive.add_path(Path::new(&project.summary_file))
            .expect("Fail to add the summary file to the zip archive");
        archive.add_buf(format!("{:#?}", project).as_bytes(), Path::new("configuration.ron"))
            .expect("Fail to add the configuration file to the zip archive");

        archive.finish()
            .expect("Fail to build the archive");
    }

}
