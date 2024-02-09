use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
};

use log::{debug, error, info};

pub const TEST_PATH: &'static str = env!(
    "LOC_JULEA_TESTS",
    "Missing environment variable 'LOC_TEST_DIRECTORY'."
);

pub const WRITE_FILE: &'static str = "write.txt";
pub const READ_FILE: &'static str = "read.txt";
pub const DELETE_FILE: &'static str = "delete.txt";
#[allow(dead_code)]
pub const CREATE_FILE: &'static str = "create.txt";

pub fn setup() {
    info!("Preparing test environment...");

    match prepare_test_directory() {
        Ok(_) => debug!("Test directory prepared."),
        Err(e) => {
            error!(
                "An error occured while setting up the test directory: {}, {}",
                e.kind(),
                e.to_string()
            );
            panic!("Failed to set up the test environment!");
        }
    }

    info!("Test environment ready.");
}

fn prepare_test_directory() -> std::io::Result<()> {
    let _ = fs::remove_dir_all(TEST_PATH);
    fs::create_dir_all(PathBuf::new().join(TEST_PATH))?;
    debug!("Reset test directory \"{TEST_PATH}\"");

    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(get_path(WRITE_FILE))?;
    debug!("Created {WRITE_FILE}.");

    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(get_path(DELETE_FILE))?;
    debug!("Created {DELETE_FILE}.");

    let mut hello_file: File = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(get_path(READ_FILE))?;
    hello_file.write("Hello, world!".as_bytes())?;
    debug!("Created {READ_FILE}.");

    Ok(())
}

fn get_path(file_name: &str) -> PathBuf {
    PathBuf::new().join(TEST_PATH).join(file_name)
}
