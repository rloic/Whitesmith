mod model;
mod tools;

use std::{thread};
use std::fs::File;
use std::io::{BufReader, BufRead, stdout, Write, stdin, BufWriter};
use std::path::{Path, PathBuf};

use crate::model::project::Project;
use clap::{App, Arg, Values};
use crate::model::{working_directory, source_directory, log_directory, summary_file, zip_file};
use std::sync::Arc;
use crate::tools::RecursiveZipWriter;
use zip::CompressionMethod;
use ron::ser::PrettyConfig;
use std::ffi::OsStr;
use std::collections::HashSet;
use crate::model::commands::restore_path;
use termimad::MadSkin;
use crossterm::style::Color;
use std::process::{Command, Stdio};
use std::cmp::Ordering;

extern crate wait_timeout;
extern crate serde;
extern crate ron;
extern crate humantime;

const CONFIG_ARG: &str = "CONFIG";
const RUN_FLAG: &str = "run";
const BUILD_FLAG: &str = "build";
const CLEAN_FLAG: &str = "clean";
const WITH_IN_PROGRESS_FLAG: &str = "with-in-progress";
const WITH_TIMEOUT_FLAG: &str = "with-timed-out";
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
const SUMMARY_FLAG: &str = "summary";
const EDIT_ARG: &str = "edit";
const SORT_ARG: &str = "sort";

fn check_nb_thread(v: String) -> Result<(), String> {
    if let Ok(number) = v.parse::<usize>() {
        if number < 1 {
            Err(String::from("The number of threads must be strictly positive"))
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

fn optional_single_argument(name: &str) -> Arg {
    Arg::with_name(name)
        .takes_value(true)
        .multiple(false)
}

fn required_single_argument(name: &str) -> Arg {
    optional_single_argument(name)
        .required(true)
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
            .help("Override the configuration shortcuts with custom value (usage: --override key:value)"))
        .arg(flag(DEBUG_FLAG)
            .long(DEBUG_FLAG)
            .short("d")
            .help("Run the experiments in debug mode, i.e. exit the executions on the first failure"))
        .arg(optional_single_argument(NB_THREADS_ARG)
            .long(NB_THREADS_ARG)
            .help("Set the number of parallel threads (default=1)")
            .validator(check_nb_thread))
        .arg(optional_single_argument(GLOBAL_TIMEOUT_ARG)
            .long(GLOBAL_TIMEOUT_ARG)
            .short("T")
            .help("Override (or set) the global timeout")
            .validator(check_global_timeout))
        .arg(flag(WITH_IN_PROGRESS_FLAG)
            .long(WITH_IN_PROGRESS_FLAG)
            .help("Allows to re-run the experiments that weren't finished in the previous call"))
        .arg(flag(WITH_TIMEOUT_FLAG)
            .long(WITH_TIMEOUT_FLAG)
            .help("Allows to re-run the experiments that reach the timeout in the previous call"))
        .arg(flag(WITH_FAILURE_FLAG)
            .long(WITH_FAILURE_FLAG)
            .help("Allows to re-run the experiments that failed in the previous call"))
        .arg(flag(ZIP_FLAG)
            .long(ZIP_FLAG)
            .help("Zip the logs into an archive at the end of the computation"))
        .arg(optional_multiple_arguments(ZIP_WITH_FLAG)
            .long(ZIP_WITH_FLAG)
            .help("Add the files to the zip archive"))
        .arg(flag(STATUS_FLAG)
            .long(STATUS_FLAG)
            .short("s")
            .help("Print the status of each experiment"))
        .arg(optional_multiple_arguments(ONLY_FLAG)
            .long(ONLY_FLAG)
            .help("Run only the experiments that matches the names given as argument"))
        .arg(flag(NOTES_FLAG)
            .long(NOTES_FLAG)
            .help("Display the notes (description) of the configuration file"))
        .arg(optional_single_argument(CONFIGURATION_ARG)
            .long(CONFIGURATION_ARG)
            .help("Use a configuration file to override the configuration shortcuts. If --override is also used --override will get the priority"))
        .arg(flag(SUMMARY_FLAG)
            .long(SUMMARY_FLAG)
            .help("Display the summary file if available")
        )
        .arg(optional_single_argument(EDIT_ARG)
            .long(EDIT_ARG)
            .help("Edit the configuration file"))
        .arg(optional_multiple_arguments(SORT_ARG)
            .long(SORT_ARG)
            .help("Sort summary content"))
        .get_matches();

    let path = matches.value_of("CONFIG").unwrap();
    assert!(path.ends_with(".zip") || path.ends_with(".ron"));
    let path = Path::new(path);

    if let Some(text_editor) = matches.value_of(EDIT_ARG) {
        Command::new(text_editor)
            .arg(path)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .expect(&format!("Cannot execute {}", text_editor));
        return;
    }

    let config_file = File::open(path)
        .expect(&format!("Cannot open the configuration file '{:?}'. Maybe the file doesn't exists or the permissions are too restrictive.", path));

    let (mut project, is_zip_archive) = if path.extension() == Some(OsStr::new("zip")) {
        let mut archive = zip::ZipArchive::new(config_file)
            .expect("Cannot read the zip file");
        let zip_config_file = archive.by_name("configuration.ron")
            .expect("Cannot read the configuration.ron file. Maybe the archive wasn't build by whitesmith");
        (ron::de::from_reader::<_, Project>(BufReader::new(zip_config_file))
            .map_err(|e| e.to_string())
            .expect("Cannot parse the configuration file"), true)
    } else {
        (ron::de::from_reader::<_, Project>(BufReader::new(config_file))
            .map_err(|e| e.to_string())
            .expect("Cannot parse the configuration file"), false)
    };

    project.working_directory = working_directory(path, &project.versioning);
    println!("{}", project.working_directory);
    project.source_directory = source_directory(path, &project.versioning);
    project.log_directory = log_directory(path, &project.versioning);
    project.summary_file = summary_file(path, &project.versioning, is_zip_archive);
    project.debug = matches.is_present(DEBUG_FLAG);

    project.shortcuts.insert(String::from("PROJECT"), project.working_directory.to_owned());
    project.shortcuts.insert(String::from("SOURCES"), project.source_directory.to_owned());
    project.shortcuts.insert(String::from("LOGS"), project.log_directory.to_owned());
    project.shortcuts.insert(String::from("SUMMARY_FILE"), project.summary_file.to_owned());

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
        if Path::new(&project.summary_file).exists() {
            let valid_answers = ["", "y", "Y", "n", "N"];
            let mut answer = String::new();
            loop {
                print!("The project has been executed. Would you save the previous results before cleaning the project ? [Y/n] ");
                stdout().flush().unwrap();
                stdin().read_line(&mut answer).expect("Cannot read stdin");
                let answer = answer.trim();
                if valid_answers.iter().any(|&it| it == answer) {
                    break;
                }
            }

            let positive_answers = &valid_answers[0..3];
            let answer = answer.trim();
            if positive_answers.contains(&answer) {
                let zip_path = zip_path.replace(".zip", ".backup.zip");
                zip_project(&zip_path, project.as_ref(), &mut matches.values_of(ZIP_WITH_FLAG));
            }
        }
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
        if let Ok(file) = File::create(Path::new(&project.working_directory).join("last_running_configuration.ron")) {
            let writer = BufWriter::new(file);
            ron::ser::to_writer_pretty(writer, project.as_ref(), PrettyConfig::default())
                .expect("Cannot serialize the project file to toml");
        }

        run_project(
            project.clone(),
            matches.value_of(NB_THREADS_ARG),
            selected_instances.as_ref(),
            matches.is_present(WITH_IN_PROGRESS_FLAG),
            matches.is_present(WITH_TIMEOUT_FLAG),
            matches.is_present(WITH_FAILURE_FLAG),
        );
    }

    if matches.is_present(STATUS_FLAG) {
        project.display_status(selected_instances.as_ref());
    }

    if matches.is_present(ZIP_FLAG) {
        zip_project(&zip_path, project.as_ref(), &mut matches.values_of(ZIP_WITH_FLAG));
    }

    if matches.is_present(NOTES_FLAG) {
        print_notes(project.as_ref());
    }

    if matches.is_present(SUMMARY_FLAG) {
        println!("{}", &project.summary_file);
        let sort_columns = matches.values_of(SORT_ARG).map(|it| it.collect::<Vec<_>>());
        let result = if is_zip_archive {
            let mut archive = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
            let summary_file = archive.by_name(&project.summary_file).unwrap();
            let mut reader = BufReader::new(summary_file);
            print_summary(&mut reader, sort_columns)
        } else {
            if let Ok(summary_file) = File::open(&project.summary_file) {
                let mut reader = BufReader::new(summary_file);
                print_summary(&mut reader, sort_columns)
            } else {
                Ok(())
            }
        };
        result.expect("Cannot read the summary file");
    }
}

fn print_summary<RS>(reader: &mut BufReader<RS>, sort_columns: Option<Vec<&str>>) -> std::io::Result<()>
    where RS: std::io::Read {
    let mut col_sizes = Vec::new();
    let mut lines = Vec::new();

    let mut headers = None;

    for line in reader.lines() {
        let line = line?;
        let parts = line.split('\t')
            .map(String::from)
            .collect::<Vec<_>>();
        if let None = headers {
            headers = Some(parts.clone());
        }
        let parts_len = parts.iter()
            .map(&String::len)
            .collect::<Vec<_>>();
        let mut i = 0;
        while i < usize::min(col_sizes.len(), parts.len()) {
            col_sizes[i] = usize::max(col_sizes[i], parts_len[i]);
            i += 1;
        }

        while col_sizes.len() < parts.len() {
            col_sizes.push(parts_len[i]);
            i += 1;
        }
        lines.push(parts);
    }

    if let Some(header) = headers {
        if let Some(sort_columns) = sort_columns {
            let empty_string = String::new();
            lines[1..].sort_by(|lhs, rhs| {
                for column in &sort_columns {

                    let (column, rev) = if column.starts_with('~') {
                        (column.chars().skip(1).collect::<String>(), true)
                    } else {
                        (column.to_string(), false)
                    };

                    if let Some(index) = header.iter().position(|it| it.eq_ignore_ascii_case(&column)) {
                        let mut comparison = human_sort::compare(
                            &lhs.get(index).unwrap_or(&empty_string),
                            &rhs.get(index).unwrap_or(&empty_string)
                        );

                        if rev { comparison = comparison.reverse(); }

                        if comparison != Ordering::Equal {
                            return comparison;
                        }
                    }
                }
                Ordering::Equal
            });
        }
    }

    for line in lines {
        for (i, part) in line.iter().enumerate() {
            print!("{:1$}", part, col_sizes[i] + 3);
        }
        println!();
    }

    Ok(())
}

fn zip_project(zip_path: &str, project: &Project, files_to_add: &mut Option<Values>) {
    let zip_file = File::create(zip_path)
        .expect("Cannot create the zip archive");
    let mut archive = RecursiveZipWriter::new(zip_file)
        .compression_method(CompressionMethod::Stored);

    let mut paths = HashSet::new();

    archive.add_path(Path::new(&project.log_directory))
        .expect("Fail to add the log directory to the zip archive");
    paths.insert(PathBuf::from(&project.log_directory));

    archive.add_path(Path::new(&project.summary_file))
        .expect("Fail to add the summary file to the zip archive");
    paths.insert(PathBuf::from(&project.summary_file));

    archive.add_path(Path::new(&project.working_directory).join("last_running_configuration.ron").as_path())
        .expect("Cannot add the running configuration file to the zip archive");
    paths.insert(PathBuf::from(&project.working_directory).join("last_running_configuration.ron"));

    let serialized_project = ron::ser::to_string_pretty(project, PrettyConfig::default())
        .expect("Cannot serialize the project file to toml");
    archive.add_buf(serialized_project.as_bytes(), Path::new("configuration.ron"))
        .expect("Fail to add the configuration file to the zip archive");
    paths.insert(PathBuf::from("configuration.ron"));

    for file_to_add in &project.zip_with {
        let full_path = restore_path(&PathBuf::from(&file_to_add), &project.shortcuts);
        if !paths.contains(&full_path) {
            archive.add_path(&full_path)
                .expect(&format!("Fail to add {} to the zip archive", file_to_add));
            paths.insert(full_path);
        }
    }
    if let Some(files_to_add) = files_to_add {
        for file_to_add in files_to_add {
            let full_path = restore_path(&PathBuf::from(&file_to_add), &project.shortcuts);
            if !paths.contains(&full_path) {
                archive.add_path(&full_path)
                    .expect(&format!("Fail to add {} to the zip archive", file_to_add));
                paths.insert(full_path);
            }
        }
    }

    let archive = archive.finish()
        .expect("Fail to build the archive");

    println!("{:?}", archive);
}

fn print_notes(project: &Project) {
    if let Some(description) = &project.description {
        let mut description = description.trim().to_owned();

        description.insert_str(0, "\n---\n");
        description.push_str("\n---\n");

        let mut skin = MadSkin::default_dark();
        skin.bold.set_fg(Color::Red);
        skin.print_text(&description);

        // println!("{}", &description);
    } else {
        println!("The configuration doesn't contain notes.")
    }
}

fn run_project(
    project: Arc<Project>,
    nb_threads: Option<&str>,
    selected_instances: &Option<Vec<String>>,
    with_in_progress: bool,
    with_timeout: bool,
    with_failure: bool,
) {
    if project.requires_overrides() {
        return;
    }

    if with_in_progress {
        project.unlock_in_progress();
    }

    if with_timeout {
        project.unlock_timeout();
    }

    if with_failure {
        project.unlock_failed();
    }

    if let Some(nb_threads) = nb_threads {
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