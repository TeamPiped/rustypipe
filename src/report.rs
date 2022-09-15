use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::{DateTime, Local};
use log::error;
use serde::{Deserialize, Serialize};

use crate::deobfuscate::DeobfData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// Rust package name (`rustypipe`)
    pub package: String,
    /// Package version (`0.1.0`)
    pub version: String,
    /// Date/Time when the event occurred
    pub date: DateTime<Local>,
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

pub trait Reporter {
    fn report(&self, report: &Report);
}

pub struct JsonFileReporter {
    path: PathBuf,
}

impl JsonFileReporter {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    fn _report(&self, report: &Report) -> Result<()> {
        let report_path = get_report_path(&self.path, report)?;
        serde_json::to_writer_pretty(&File::create(report_path)?, &report)?;
        Ok(())
    }
}

impl Default for JsonFileReporter {
    fn default() -> Self {
        Self {
            path: Path::new("rustypipe_reports").to_path_buf(),
        }
    }
}

impl Reporter for JsonFileReporter {
    fn report(&self, report: &Report) {
        self._report(report)
            .unwrap_or_else(|e| error!("Could not store report file. Err: {}", e));
    }
}

#[cfg(feature = "yaml")]
pub struct YamlFileReporter {
    path: PathBuf,
}

#[cfg(feature = "yaml")]
impl YamlFileReporter {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    fn _report(&self, report: &Report) -> Result<()> {
        let report_path = get_report_path(&self.path, report)?;
        serde_yaml::to_writer(&File::create(report_path)?, &report)?;
        Ok(())
    }
}

#[cfg(feature = "yaml")]
impl Default for YamlFileReporter {
    fn default() -> Self {
        Self {
            path: Path::new("rustypipe_reports").to_path_buf(),
        }
    }
}

#[cfg(feature = "yaml")]
impl Reporter for YamlFileReporter {
    fn report(&self, report: &Report) {
        self._report(report)
            .unwrap_or_else(|e| error!("Could not store report file. Err: {}", e));
    }
}

fn get_report_path(root: &Path, report: &Report) -> Result<PathBuf> {
    if !root.is_dir() {
        std::fs::create_dir_all(root)?;
    }

    let filename_prefix = format!("{}_{:?}", report.date.format("%F_%H-%M-%S"), report.level);

    let mut report_path = root.to_path_buf();
    report_path.push(format!("{}.yaml", filename_prefix));

    // ensure unique filename
    for i in 1..u32::MAX {
        if report_path.exists() {
            report_path = root.to_path_buf();
            report_path.push(format!("{}_{}.yaml", filename_prefix, i));
        } else {
            break;
        }
    }

    Ok(report_path)
}
