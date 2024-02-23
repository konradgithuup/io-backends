use assert_fs::{
    fixture::{FileTouch, FileWriteStr, PathChild, PathCreateDir},
    TempDir,
};
use log::{debug, error, info};
pub const WRITE_FILE: &'static str = "write.txt";
pub const READ_FILE: &'static str = "read.txt";
pub const DELETE_FILE: &'static str = "delete.txt";
#[allow(dead_code)]
pub const CREATE_FILE: &'static str = "create.txt";

pub fn setup() -> TempDir {
    info!("Preparing test environment...");

    let temp = match prepare_test_directory() {
        Ok(temp) => {
            debug!("Test directory prepared.");
            temp
        }
        Err(e) => {
            error!(
                "An error occured while setting up the test directory: {}",
                e
            );
            panic!("Failed to set up the test environment!");
        }
    };

    info!("Test environment ready.");
    temp
}

pub fn shutdown(temp: TempDir) {
    info!("Deleting test environment...");
    temp.close().unwrap();
}

fn prepare_test_directory() -> std::result::Result<TempDir, String> {
    let temp = TempDir::new().map_err(|e| e.to_string())?;
    debug!(
        "Test directory: \"{:?}\"",
        temp.to_str().unwrap_or("[cannot display]")
    );

    temp.child(WRITE_FILE).touch().map_err(|e| e.to_string())?;
    temp.child(DELETE_FILE).touch().map_err(|e| e.to_string())?;
    temp.child(READ_FILE)
        .write_str("Hello, world!")
        .map_err(|e| e.to_string())?;

    temp.child("subdir")
        .create_dir_all()
        .map_err(|e| e.to_string())?;
    temp.child("subdir/prefix_a.txt")
        .touch()
        .map_err(|e| e.to_string())?;
    temp.child("subdir/prefix_b.txt")
        .touch()
        .map_err(|e| e.to_string())?;
    temp.child("subdir/c.txt")
        .touch()
        .map_err(|e| e.to_string())?;

    debug!("Test directory populated.");

    Ok(temp)
}
