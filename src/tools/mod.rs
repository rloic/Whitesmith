use zip::{ZipWriter, CompressionMethod};
use zip::write::FileOptions;
use zip::result::ZipResult;
use zip::result::ZipError;

use std::io::Write;
use std::io::Seek;

use std::path::Path;

use std::fs::{File, DirEntry};
use std::ffi::{OsString};


pub struct RecursiveZipWriter<W: Write + Seek> {
    zip_writer: ZipWriter<W>,
    options: FileOptions
}

fn is_ignored(entry: &DirEntry, ignored: &Vec<String>) -> bool {
    let file_name = entry.file_name();
    let path = entry.path();
    let extension = path.extension();

    for ignore in ignored {
        if !ignore.is_empty() {
            if ignore.starts_with("*.") {
                let ignored_extension = &ignore[2..];
                if extension == Some(&OsString::from(ignored_extension)) {
                    return true;
                }
            } else {
                if file_name == OsString::from(ignore) {
                    return true;
                }
            }
        }
    }

    false
}

impl<W: Write + Seek> RecursiveZipWriter<W> {
    pub fn new(inner: W) -> Self {
        RecursiveZipWriter { zip_writer: ZipWriter::new(inner), options: FileOptions::default() }
    }

    pub fn add_path_renamed_with_exception(&mut self, real_path: &Path, zip_path: &Path, ignored: &Vec<String>) -> Result<(), ZipError> {
        if real_path.is_file() {
            self.zip_writer
                .start_file(zip_path.to_string_lossy().into_owned(), self.options)?;
            let mut file = File::open(real_path).unwrap();
            std::io::copy(&mut file, &mut self.zip_writer)?;
        } else if real_path.is_dir() {
            for listing in real_path.read_dir().unwrap() {
                let file = listing.unwrap();
                let file_name = file.file_name();

                if !is_ignored(&file, &ignored) {
                    self.add_path_renamed_with_exception(&real_path.join(&file_name), &zip_path.join(&file_name), ignored)
                        .unwrap_or(());
                }
            }
        } else {
            println!("Cannot add {:?} to the current archive", real_path);
        }
        Ok(())
    }

    pub fn add_path_renamed(&mut self, real_path: &Path, zip_path: &Path) -> Result<(), ZipError> {
        if real_path.is_file() {
            self.zip_writer
                .start_file(zip_path.to_string_lossy().into_owned(), self.options)?;
            let mut file = File::open(real_path).unwrap();
            std::io::copy(&mut file, &mut self.zip_writer)?;
        } else if real_path.is_dir() {
            for listing in real_path.read_dir().unwrap() {
                let file_name = listing.unwrap().file_name();
                self.add_path_renamed(&real_path.join(&file_name), &zip_path.join(&file_name))
                    .unwrap_or(());
            }
        } else {
            println!("Cannot add {:?} to the current archive", real_path);
        }
        Ok(())
    }

    pub fn add_buf(&mut self, buf: &[u8], zip_path: &Path) -> Result<(), ZipError> {
        self.zip_writer
            .start_file(zip_path.to_string_lossy().into_owned(), self.options)?;
        self.zip_writer.write_all(buf)?;
        Ok(())
    }

    pub fn add_path_with_exception(&mut self, real_path: &Path, exceptions: &Vec<String>) -> Result<(), ZipError> {
        self.add_path_renamed_with_exception(real_path, &Path::new(real_path.file_name().unwrap()), exceptions)
    }

    pub fn add_path(&mut self, real_path: &Path) -> Result<(), ZipError> {
        self.add_path_renamed(real_path, &Path::new(real_path.file_name().unwrap()))
    }

    pub fn finish(&mut self) -> ZipResult<W> {
        self.zip_writer.finish()
    }

    pub fn compression_method(self, method: CompressionMethod) -> Self {
        self.options.compression_method(method);
        self
    }
}