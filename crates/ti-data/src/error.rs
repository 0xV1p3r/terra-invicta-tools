use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parsing {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("unsupported data format version {found} (this library reads {supported})")]
    FormatVersion { found: u32, supported: u32 },
    #[error("build {0:?} not found in data root")]
    UnknownBuild(String),
    #[error("data root contains no extracted builds")]
    NoBuilds,
    #[error("language {0:?} is not part of this build")]
    UnknownLanguage(String),
    #[error("build has {count} template layers; at most {max} are supported")]
    TooManyLayers { count: usize, max: usize },
    #[error("{template} {data_name:?} has {count} records; at most {max} are supported")]
    TooManyVariants {
        template: String,
        data_name: String,
        count: usize,
        max: usize,
    },
    #[error("build has {count} scenarios; at most {max} are supported")]
    TooManyScenarios { count: usize, max: usize },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
