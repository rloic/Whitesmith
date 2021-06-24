mod model;
mod tools;

use std::{thread};
use std::fs::File;
use std::io::{BufReader, BufRead};
use std::path::{Path, PathBuf};

use crate::model::project::Project;
use clap::{App, Arg};
use crate::model::{working_directory, source_directory, log_directory, summary_file, zip_file};
use std::sync::Arc;
use crate::tools::RecursiveZipWriter;
use zip::CompressionMethod;
use ron::ser::PrettyConfig;
use std::ffi::OsStr;

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
const ZIP_WITH_FLAG: &str = "zip-with";
const STATUS_FLAG: &str = "status";
const ONLY_FLAG: &str = "only";
const NOTES_FLAG: &str = "notes";
const CONFIGURATION_ARG: &str = "config";

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
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
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
        .arg(optional_multiple_arguments(ZIP_WITH_FLAG)
            .long(ZIP_WITH_FLAG)
            .help("Add the files to the zip archive")
        )
        .arg(flag(STATUS_FLAG)
            .long(STATUS_FLAG)
            .short("s")
            .help("Print the status of each experiment")
        )
        .arg(optional_multiple_arguments(ONLY_FLAG)
            .long(ONLY_FLAG)
            .help("Run only the experiments that matches the names given as argument")
        )
        .arg(flag(NOTES_FLAG)
            .long(NOTES_FLAG)
            .help("Display the notes (description) of the configuration file")
        )
        .arg(optional_single_argument(CONFIGURATION_ARG)
            .long(CONFIGURATION_ARG)
            .help("Use a configuration file to override the configuration shortcuts. If --override is also used --override will get the priority")
        ).get_matches();

    let path = matches.value_of("CONFIG").unwrap();
    assert!(path.ends_with(".zip") || path.ends_with(".ron"));
    let path = Path::new(path);

    let config_file = File::open(path)
        .expect(&format!("Cannot open the configuration file '{:?}'. Maybe the file doesn't exists or the permissions are too restrictive.", path));

    let mut project = if path.extension() == Some(OsStr::new("zip")) {
        let mut archive = zip::ZipArchive::new(config_file)
            .expect("Cannot read the zip file");
        let zip_config_file = archive.by_name("configuration.ron")
            .expect("Cannot read the configuration.ron file. Maybe the archive wasn't build by whitesmith");
        ron::de::from_reader::<_, Project>(BufReader::new(zip_config_file))
            .map_err(|e| e.to_string())
            .expect("Cannot parse the configuration file")
    } else {
        ron::de::from_reader::<_, Project>(BufReader::new(config_file))
            .map_err(|e| e.to_string())
            .expect("Cannot parse the configuration file")
    };

    project.working_directory = working_directory(path);
    project.source_directory = source_directory(path);
    project.log_directory = log_directory(path);
    project.summary_file = summary_file(path);
    project.debug = matches.is_present(DEBUG_FLAG);

    let zip_path = zip_file(path, &project);

    if let Some(path) = matches.value_of(CONFIGURATION_ARG) {
        let file = File::open(path)
            .expect(&format!("Cannot open configuration file {}", path));

        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line.unwrap();
            let fields = line.split(':').collect::<Vec<_>>();
            let (key, value) = (fields[0], fields[1]);
            project.shortcuts.insert(key.to_owned(), value.to_owned());
        }
    }

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

    let selected_instances = matches.values_of(ONLY_FLAG).map(|values| {
        let mut instances = Vec::new();
        for value in values {
            instances.push(value.to_owned());
        }
        instances
    });
    let selected_instances = Arc::new(selected_instances);

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
                let selected_instances = selected_instances.clone();
                handlers.push(thread::spawn(move || { project.run(&selected_instances) }));
            }
            for handler in handlers { handler.join().unwrap(); }
        } else {
            project.run(&selected_instances);
        }
    }

    if matches.is_present(STATUS_FLAG) {
        project.display_status(&selected_instances);
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
        let serialized_project = ron::ser::to_string_pretty(project.as_ref(), PrettyConfig::default())
            .expect("Cannot serialize the project file to toml");
        archive.add_buf(serialized_project.as_bytes(), Path::new("configuration.ron"))
            .expect("Fail to add the configuration file to the zip archive");
        if let Some(files_to_add) = matches.values_of(ZIP_WITH_FLAG) {
            for file_to_add in files_to_add {
                archive.add_path(&PathBuf::from(&project.working_directory).join(file_to_add))
                    .expect(&format!("Fail to add {} to the zip archive", file_to_add));
            }
        }

        let archive = archive.finish()
            .expect("Fail to build the archive");

        println!("{:?}", archive);
    }

    if matches.is_present(NOTES_FLAG) {
        if let Some(description) = &project.description {
            let mut description = description.trim().to_owned();

            description.insert_str(0, "\n---\n");
            description.push_str("\n---\n");

            println!("{}", &description);
        } else {
            println!("The configuration doesn't contain notes.")
        }
    }
}