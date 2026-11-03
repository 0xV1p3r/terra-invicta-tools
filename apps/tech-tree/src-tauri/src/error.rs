//! Error type of the commands: serialized to the frontend as a plain message.

/// Error returned to the frontend as a plain message.
#[derive(Debug, thiserror::Error)]
pub enum CmdError {
    #[error("{0}")]
    Data(#[from] ti_data::Error),
    #[error("{0}")]
    Other(String),
}

impl CmdError {
    pub fn msg(m: impl Into<String>) -> Self {
        CmdError::Other(m.into())
    }
}

impl serde::Serialize for CmdError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type CmdResult<T> = Result<T, CmdError>;
