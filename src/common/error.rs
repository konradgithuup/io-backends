#![allow(dead_code)]

use std::{error::Error, ffi::NulError, fmt::Display};

pub type Result<T> = std::result::Result<T, BackendError>;

#[derive(Debug)]
pub struct BackendError {
    msg: String,
    action: Action,
}

impl Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "BackendError in {:?}: {}",
            self.action, self.msg
        ))
    }
}

impl std::error::Error for BackendError {}

impl From<std::io::Error> for BackendError {
    fn from(value: std::io::Error) -> Self {
        BackendError::map(&value, Action::Internal)
    }
}

impl From<NulError> for BackendError {
    fn from(value: NulError) -> Self {
        BackendError::map(&value, Action::Internal)
    }
}

impl BackendError {
    pub fn new(msg: &str, action: Action) -> BackendError {
        BackendError {
            msg: String::from(msg),
            action,
        }
    }

    pub fn map(e: &dyn Error, action: Action) -> BackendError {
        BackendError {
            msg: e.to_string(),
            action,
        }
    }

    pub fn new_internal(msg: &str) -> BackendError {
        BackendError {
            msg: String::from(msg),
            action: Action::Internal,
        }
    }

    pub fn set_action(mut self, action: Action) -> Self {
        self.action = action;
        self
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Action {
    Init,
    Fini,
    Create,
    Delete,
    Open,
    Close,
    Status,
    Sync,
    Read,
    Write,
    Iter,
    CreateIterAll,
    CreateIterPrefix,
    Internal,
}
