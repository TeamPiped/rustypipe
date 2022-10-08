//! Error reporting

use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Local};
use log::error;
use serde::{Deserialize, Serialize};

use crate::deobfuscate::DeobfData;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Report {
    pub info: Info,
    /// Report level
    pub level: Level,
    /// RustyPipe operation (e.g. `get_player`)
    pub operation: String,
    /// Error (if occurred)
    pub error: Option<String>,
    /// Detailed error/warning messages
    pub msgs: Vec<String>,
    /// Deobfuscation data (only for player requests)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deobf_data: Option<DeobfData>,
    /// HTTP request data
    pub http_request: HTTPRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Info {
    /// Rust package name (`rustypipe`)
    pub package: String,
    /// Package version (`0.1.0`)
    pub version: String,
    /// Date/Time when the event occurred
    pub date: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HTTPRequest {
    /// Request URL
    pub url: String,
    /// HTTP method
    pub method: String,
    /// HTTP request header
    pub req_header: BTreeMap<String, String>,
    /// HTTP request body
    pub req_body: String,
    /// HTTP response status code
    pub status: u16,
    /// HTTP response body
    pub resp_body: String,
}

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

impl Default for Info {
    fn default() -> Self {
        Self {
            package: "rustypipe".to_owned(),
            version: "0.1.0".to_owned(),
            date: chrono::Local::now(),
        }
    }
}

pub trait Reporter: Sync + Send {
    fn report(&self, report: &Report);
}

pub struct FileReporter {
    path: PathBuf,
}

impl FileReporter {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    fn _report(&self, report: &Report) -> Result<()> {
        let report_path = get_report_path(&self.path, report, "json")?;
        serde_json::to_writer_pretty(&File::create(report_path)?, &report).or_else(|e| {
            Err(Error::Other(
                format!("could not serialize report. err: {}", e).into(),
            ))
        })?;
        Ok(())
    }
}

impl Default for FileReporter {
    fn default() -> Self {
        Self {
            path: Path::new("rustypipe_reports").to_path_buf(),
        }
    }
}

impl Reporter for FileReporter {
    fn report(&self, report: &Report) {
        self._report(report)
            .unwrap_or_else(|e| error!("Could not store report file. Err: {}", e));
    }
}

fn get_report_path(root: &Path, report: &Report, ext: &str) -> Result<PathBuf> {
    if !root.is_dir() {
        std::fs::create_dir_all(root)?;
    }

    let filename_prefix = format!(
        "{}_{:?}",
        report.info.date.format("%F_%H-%M-%S"),
        report.level
    );

    let mut report_path = root.to_path_buf();
    report_path.push(format!("{}.{}", filename_prefix, ext));

    // ensure unique filename
    for i in 1..u32::MAX {
        if report_path.exists() {
            report_path = root.to_path_buf();
            report_path.push(format!("{}_{}.{}", filename_prefix, i, ext));
        } else {
            break;
        }
    }

    Ok(report_path)
}
