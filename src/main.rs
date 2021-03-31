mod model;

use std::{io, thread};
use std::fs::File;
use std::io::{ErrorKind, BufReader};
use std::path::Path;

use crate::model::project::Project;
use clap::{App, Arg};
use crate::model::{working_directory, source_directory, log_directory, summary_file};
use std::sync::Arc;

extern crate wait_timeout;
extern crate serde;
extern crate ron;
extern crate humantime;

const CONFIG_ARG: &str = "CONFIG";
const RUN_FLAG: &str = "run";
const BUILD_FLAG: &str = "build";
const CLEAN_FLAG: &str = "clean";
const GIT_FLAG: &str = "git";
const OVERRIDE: &str = "override";
const DEBUG: &str = "debug";
const NB_THREADS: &str = "nb_threads";

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

fn main() -> io::Result<()> {
    let matches = App::new("whitesmith")
        .version("0.1")
        .author("Loïc Rouquette <loic.rouquette@insa-lyon.fr>")
        .arg(Arg::with_name(CONFIG_ARG)
            .help("Configuration file")
            .required(true)
            .index(1))
        .arg(Arg::with_name(RUN_FLAG)
            .long(RUN_FLAG)
            .short("r")
            .help("Run the experiments"))
        .arg(Arg::with_name(GIT_FLAG)
            .long(GIT_FLAG)
            .short("g")
            .help("Fetch sources from the git repository"))
        .arg(Arg::with_name(BUILD_FLAG)
            .long(BUILD_FLAG)
            .short("b")
            .help("Build the project from sources (must be downloaded before)"))
        .arg(Arg::with_name(CLEAN_FLAG)
            .long(CLEAN_FLAG)
            .help("Remove previous experiments results"))
        .arg(Arg::with_name(OVERRIDE)
            .long(OVERRIDE)
            .help("Override the configuration shortcuts with custom value (usage: --override key:value)")
            .takes_value(true)
            .multiple(true)
        )
        .arg(Arg::with_name(DEBUG)
            .long(DEBUG)
            .short("d")
            .help("Run the experiments in debug mode, i.e. exit the executions on the first failure")
        )
        .arg(Arg::with_name(NB_THREADS)
            .long(NB_THREADS)
            .help("Set the number of parallel threads (default=1)")
            .takes_value(true)
            .validator(check_nb_thread)
        )
        .get_matches();


    let path = matches.value_of("CONFIG").unwrap();
    let path = Path::new(path);
    let config_file = File::open(path)?;
    let mut project = ron::de::from_reader::<_, Project>(BufReader::new(config_file))
        .map_err(|e| io::Error::new(ErrorKind::InvalidInput, e))?;

    project.working_directory = working_directory(path);
    project.source_directory = source_directory(path);
    project.log_directory = log_directory(path);
    project.summary_file = summary_file(path);
    project.debug = matches.is_present(DEBUG);

    if let Some(mut values) = matches.values_of(OVERRIDE) {
        while let Some(value) = values.next() {
            let fields = value.split(':').collect::<Vec<_>>();
            project.shortcuts.insert(fields[0].to_owned(), fields[1].to_owned());
        }
    }

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
        if let Some(nb_threads) = matches.value_of(NB_THREADS) {
            let nb_threads = nb_threads.parse::<usize>().unwrap();

            let mut handlers = Vec::with_capacity(nb_threads);
            let project = Arc::new(project);
            for _ in 0..nb_threads {
                let project = project.clone();
                handlers.push(thread::spawn(move || { project.run() }));
            }

            for handler in handlers {
                handler.join()
                    .unwrap();
            }

        } else {
            project.run();
        }
    }

    Ok(())
}

