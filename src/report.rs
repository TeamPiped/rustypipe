//! # Error reporting
//!
//! Due to the instability of the Innertube API, RustyPipe may not be able to parse
//! every item from every YouTube response. To allow for easy debugging, RustyPipe
//! can create and store error reports.
//!
//! These reports contain information about the RustyPipe client, the performed
//! operation, the request sent to YouTube and the received response data.
//!
//! With the report data the error can be reproduced and RustyPipe can be patched to
//! handle YouTube's changes to the response model.
//!
//! By default, RustyPipe stores the reports as JSON files
//! (e.g `rustypipe_reports/2022-11-05_22-58-59_ERR`).
//!
//! By implementing the [`Reporter`] trait you can handle error reports in other ways
//! (e.g. store them in a database, send them via mail, log to Sentry, etc).

use std::{
    collections::BTreeMap,
    fs::File,
    io::Error,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use time::{macros::format_description, OffsetDateTime};
use tracing::error;

use crate::{deobfuscate::DeobfData, param::Language, util};

pub(crate) const DEFAULT_REPORT_DIR: &str = "rustypipe_reports";

const FILENAME_FORMAT: &[time::format_description::FormatItem] =
    format_description!("[year]-[month]-[day]_[hour]-[minute]-[second]");

/// RustyPipe error report
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Report<'a> {
    /// Information about the RustyPipe client
    pub info: RustyPipeInfo<'a>,
    /// Severity of the report
    pub level: Level,
    /// RustyPipe operation (e.g. `get_player`)
    pub operation: &'a str,
    /// Error (if occurred)
    pub error: Option<String>,
    /// Detailed error/warning messages
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub msgs: Vec<String>,
    /// Deobfuscation data (only for player requests)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deobf_data: Option<DeobfData>,
    /// HTTP request data
    pub http_request: HTTPRequest<'a>,
}

/// Information about the RustyPipe client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RustyPipeInfo<'a> {
    /// Rust package name (`rustypipe`)
    pub package: &'a str,
    /// Package version (`0.1.0`)
    pub version: &'a str,
    /// Date/Time when the event occurred
    #[serde(with = "time::serde::rfc3339")]
    pub date: OffsetDateTime,
    /// YouTube content language
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<Language>,
    /// RustyPipe Botguard version (`rustypipe-botguard 0.1.1`)
    pub botguard_version: Option<&'a str>,
}

/// Reported HTTP request data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HTTPRequest<'a> {
    /// Request URL
    pub url: &'a str,
    /// HTTP method
    pub method: &'a str,
    /// HTTP request header
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_header: Option<BTreeMap<&'a str, String>>,
    /// HTTP request body
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_body: Option<String>,
    /// HTTP response status code
    pub status: u16,
    /// HTTP response body
    pub resp_body: String,
}

/// Severity of the report
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// **Debug**: Operation successful, report generation was forced by setting
    /// ``.report(true)``
    DBG,
    /// **Warning**: Operation successful, but some parts could not be deserialized
    WRN,
    /// **Error**: Operation failed
    ERR,
}

impl<'a> RustyPipeInfo<'a> {
    pub(crate) fn new(language: Option<Language>, botguard_version: Option<&'a str>) -> Self {
        Self {
            package: env!("CARGO_PKG_NAME"),
            version: crate::VERSION,
            date: util::now_sec(),
            language,
            botguard_version,
        }
    }
}

/// Trait used to abstract the report storage behavior, so you can handle RustyPipe's
/// error reports in your preferred way.
pub trait Reporter: Sync + Send {
    /// Store a RustyPipe error report
    fn report(&self, report: &Report);
}

/// [`Reporter`] implementation that writes reports as JSON files to the given folder
pub struct FileReporter {
    path: PathBuf,
}

impl FileReporter {
    /// Create a new reporter that stores error reports in the given folder
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    fn _report(&self, report: &Report) -> Result<(), String> {
        let report_path = get_report_path(&self.path, report, "json").map_err(|e| e.to_string())?;
        let file = File::create(&report_path).map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(&file, &report).map_err(|e| e.to_string())?;
        tracing::warn!(
            "created report: {}",
            report_path.to_str().unwrap_or_default()
        );
        Ok(())
    }
}

impl Default for FileReporter {
    fn default() -> Self {
        Self {
            path: Path::new(DEFAULT_REPORT_DIR).to_path_buf(),
        }
    }
}

impl Reporter for FileReporter {
    fn report(&self, report: &Report) {
        self._report(report)
            .unwrap_or_else(|e| error!("Could not store report file. Err: {}", e));
    }
}

fn get_report_path(root: &Path, report: &Report, ext: &str) -> Result<PathBuf, Error> {
    if !root.is_dir() {
        std::fs::create_dir_all(root)?;
    }

    let filename_prefix = format!(
        "{}_{:?}",
        report.info.date.format(FILENAME_FORMAT).unwrap_or_default(),
        report.level
    );

    let mut report_path = root.to_path_buf();
    report_path.push(format!("{filename_prefix}.{ext}"));

    // ensure unique filename
    for i in 1..u32::MAX {
        if report_path.exists() {
            report_path = root.to_path_buf();
            report_path.push(format!("{filename_prefix}_{i}.{ext}"));
        } else {
            break;
        }
    }

    Ok(report_path)
}
