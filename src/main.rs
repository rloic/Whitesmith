mod model;
mod tools;

use std::{thread};
use std::fs::File;
use std::io::{BufReader, BufRead, stdout, Write, stdin, BufWriter};
use std::path::{Path, PathBuf};

use crate::model::project::Project;
use crate::model::{working_directory, source_directory, log_directory, summary_file, zip_file};
use std::sync::Arc;
use crate::tools::RecursiveZipWriter;
use zip::CompressionMethod;
use ron::ser::PrettyConfig;
use std::ffi::OsStr;
use std::collections::HashSet;
use crate::model::commands::restore_path;
use termimad::MadSkin;
use std::cmp::Ordering;
use clap::{Parser, Subcommand};
use termimad::crossterm::style::Color;

extern crate wait_timeout;
extern crate serde;
extern crate ron;
extern crate humantime;
extern crate clap;

fn parse_duration(v: &str) -> Result<humantime::Duration, String> {
    if let Ok(duration) = v.parse::<humantime::Duration>() {
        Ok(duration)
    } else {
        Err(format!("Cannot parse {} as Duration", v))
    }
}

#[derive(Parser)]
struct CLI {
    path: PathBuf,
    #[clap(subcommand)]
    action: Action,
    #[arg(long)]
    debug: bool,
}

#[derive(Subcommand)]
enum Action {
    Fetch(Fetch),
    Build(Build),
    Run(Run),
    Clean(Clean),
    Zip(Zip),
    Show(Show),
}

#[derive(Parser)]
struct Fetch {
    #[arg(short, long)]
    commit: Option<String>,
}

#[derive(Parser)]
struct Run {
    #[arg(short, long)]
    configuration: Option<PathBuf>,
    #[arg(short, long)]
    overrides: Vec<String>,
    #[arg(long)]
    with_failure: bool,
    #[arg(long)]
    with_in_progress: bool,
    #[arg(long)]
    with_timeout: bool,
    #[arg(short, long)]
    nb_threads: Option<usize>,
    #[arg(short, long, value_parser = parse_duration)]
    global_timeout: Option<humantime::Duration>,
    #[arg(long)]
    only: Option<Vec<String>>,
}

#[derive(Parser)]
struct Build {}

#[derive(Parser)]
struct Clean {
    #[arg(short, long)]
    zip_with: Vec<PathBuf>,
}

#[derive(Parser)]
struct Zip {
    #[arg(short, long)]
    zip_with: Vec<PathBuf>,
}

#[derive(Parser)]
struct Show {
    #[clap(subcommand)]
    action: ShowAction,
}

#[derive(Subcommand)]
enum ShowAction {
    Notes,
    Summary(Summary),
    Status(Status),
}

#[derive(Parser)]
struct Summary {
    #[arg(short, long)]
    sort: Option<Vec<String>>,
}

#[derive(Parser)]
struct Status {
    #[arg(short, long)]
    only: Option<Vec<String>>,
}

fn configure(path: &PathBuf, project: &mut Project) {
    let file = File::open(path)
        .expect(&format!("Cannot open configuration file {:?}", path));

    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line.unwrap();
        let fields = line.split(':').collect::<Vec<_>>();
        let (key, value) = (fields[0], fields[1]);
        project.shortcuts.insert(key.to_owned(), value.to_owned());
    }
}


fn main() {
    let CLI { path, action, debug } = CLI::parse();
    assert!(path.ends_with(".zip") || path.ends_with(".ron"));

    let config_file = File::open(&path)
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

    project.working_directory = working_directory(&path, &project.versioning);
    println!("{}", project.working_directory);
    project.source_directory = source_directory(&path, &project.versioning);
    project.log_directory = log_directory(&path, &project.versioning);
    project.summary_file = summary_file(&path, &project.versioning, is_zip_archive);
    project.debug = debug;

    project.shortcuts.insert(String::from("PROJECT"), project.working_directory.to_owned());
    project.shortcuts.insert(String::from("SOURCES"), project.source_directory.to_owned());
    project.shortcuts.insert(String::from("LOGS"), project.log_directory.to_owned());
    project.shortcuts.insert(String::from("SUMMARY_FILE"), project.summary_file.to_owned());

    project.init();

    let zip_path = zip_file(&path, &project);

    match action {
        Action::Fetch(fetch_args) => {
            if let Some(commit) = fetch_args.commit {
                project.versioning.commit = Some(commit);
            }
            project.fetch_sources();
        }
        Action::Build(_) => {
            project.build();
        }
        Action::Run(run_args) => {
            if let Some(path) = run_args.configuration {
                configure(&path, &mut project);
            }
            if let Ok(file) = File::create(Path::new(&project.working_directory).join("last_running_configuration.ron")) {
                let writer = BufWriter::new(file);
                ron::ser::to_writer_pretty(writer, &project, PrettyConfig::default())
                    .expect("Cannot serialize the project file to toml");
            }
            for _override in run_args.overrides {
                let fields = _override.split(':').collect::<Vec<_>>();
                let (key, value) = (fields[0], fields[1]);
                project.shortcuts.insert(key.to_owned(), value.to_owned());
            }
            if let Some(duration) = run_args.global_timeout {
                project.global_timeout = Some(duration.into());
            }
            let project = Arc::new(project);
            run_project(
                project.clone(),
                run_args.nb_threads,
                &run_args.only,
                run_args.with_in_progress,
                run_args.with_timeout,
                run_args.with_failure,
            );
        }
        Action::Clean(clean_args) => {
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
                    zip_project(&zip_path, &project, &clean_args.zip_with);
                }
            }
            project.clean();
        }
        Action::Show(show_args) => {
            match show_args.action {
                ShowAction::Notes => print_notes(&project),
                ShowAction::Summary(summary_args) => {
                    println!("{}", &project.summary_file);
                    let sort_columns = summary_args.sort;
                    let result = if is_zip_archive {
                        /*let mut archive = zip::ZipArchive::new(String::new()).unwrap();
                        let summary_file = archive.by_name(&project.summary_file).unwrap();
                        let mut reader = BufReader::new(summary_file);
                        print_summary(&mut reader, sort_columns)*/
                        Ok(())
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
                ShowAction::Status(args) => {
                    project.display_status(&args.only);
                }
            }
        }
        Action::Zip(zip) => {
            zip_project(&zip_path, &project, &zip.zip_with);
        },
    }
}

fn print_summary<RS>(reader: &mut BufReader<RS>, sort_columns: Option<Vec<String>>) -> std::io::Result<()>
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
                            &rhs.get(index).unwrap_or(&empty_string),
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

fn zip_project(zip_path: &str, project: &Project, files_to_add: &Vec<PathBuf>) {
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
    for file_to_add in files_to_add.iter() {
        let full_path = restore_path(file_to_add, &project.shortcuts);
        if !paths.contains(&full_path) {
            archive.add_path(&full_path)
                .expect(&format!("Fail to add {:?} to the zip archive", file_to_add));
            paths.insert(full_path);
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
    nb_threads: Option<usize>,
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