use crate::prelude::Action;
use crate::prelude::BackendError;
use crate::prelude::Result;

use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config::Appender;
use log4rs::config::Root;
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;

pub fn init_logger() -> Result<()> {
    let host = hostname::get()
        .map(|h| h.into_string())
        .unwrap_or(Ok(String::from("<host>")))
        .unwrap_or(String::from("<host>"));

    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            ("{d(%b %d %H:%M:%S)} ".to_owned()
                + host.as_str()
                + " julea-server[{P}]: {h({l:<5.5})} [{M}] - {m}\n")
                .as_str(),
        )))
        .build("$ENV{HOME}/log/julea-backends.log")
        .map_err(|e| BackendError::map(&e, Action::Init))?;

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            Root::builder()
                .appender("logfile")
                .build(LevelFilter::Trace),
        )
        .map_err(|e| BackendError::map(&e, Action::Init))?;

    log4rs::init_config(config).map_err(|e| BackendError::map(&e, Action::Init))?;

    Ok(())
}
