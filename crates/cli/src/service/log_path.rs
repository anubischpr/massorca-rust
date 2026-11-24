//! `massorca service log path`: the profile's logs folder and the newest service and desktop log files in it.

use massorca_net::{DESKTOP_LOG_PREFIX, SERVICE_LOG_PREFIX, newest_log_file};
use massorca_profile::Profile;
use serde::Serialize;

use crate::output::Output;

/// Where the local trace logs of a profile are. A file is `None` while that writer has logged nothing.
#[derive(Debug, Serialize)]
pub struct LogPathDocument {
    pub dir: String,
    pub service: Option<String>,
    pub desktop: Option<String>,
}

/// Prints the logs folder of `profile` and its newest files. Reads the folder only: never starts the service.
pub fn print(profile: &Profile, output: Output) {
    let dir = profile.logs_dir();
    let document = LogPathDocument {
        dir: dir.display().to_string(),
        service: newest_log_file(&dir, SERVICE_LOG_PREFIX),
        desktop: newest_log_file(&dir, DESKTOP_LOG_PREFIX),
    };
    let file_line = |label: &str, name: &Option<String>| {
        format!("{label}  {}", name.as_deref().unwrap_or("none"))
    };
    let lines = vec![
        document.dir.clone(),
        file_line(SERVICE_LOG_PREFIX, &document.service),
        file_line(DESKTOP_LOG_PREFIX, &document.desktop),
    ];
    output.success(&document, &lines);
}
