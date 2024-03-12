mod posix;

use io_backends::generate_backend;
use io_backends::prelude::*;
use log::debug;
use log::info;
use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config::Appender;
use log4rs::config::Root;
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;

generate_backend!(posix);

#[no_mangle]
pub unsafe extern "C" fn backend_info() -> *mut JBackend {
    match init_logger() {
        Ok(_) => info!("logger initialized."),
        Err(e) => {
            let _ = println!("Error while initializing logger: {e:?}");
        }
    };
    debug!("backend info called");
    &mut BACKEND
}

fn init_logger() -> Result<()> {
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
        .build("$ENV{HOME}/log/julea-backends.log")
        .map_err(|e| BackendError::map(&e, Action::Internal))?;

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            Root::builder()
                .appender("logfile")
                .build(LevelFilter::Trace),
        )
        .map_err(|e| BackendError::map(&e, Action::Internal))?;

    log4rs::init_config(config).map_err(|e| BackendError::map(&e, Action::Internal))?;

    Ok(())
}

#[cfg(test)]
mod test {
    use io_backends::prelude::*;
    use io_backends::testing::*;

    use crate::backend_info;

    #[test]
    fn test_posix_workflow() {
        let backend: ObjectBackend = unsafe {
            let b: *mut JBackend = backend_info();
            (*b).anon1.object
        };
        // let backend: ObjectBackend = unsafe { BACKEND.anon1.object };
        let data_factory = |namespace| {
            let data = PosixData {
                file_cache: FileCache::new(),
                namespace,
            };
            Box::into_raw(Box::new(data)).cast::<gpointer>()
        };
        test_workflow(&backend, &data_factory);
    }
}
