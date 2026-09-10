use serde::{Deserialize, Serialize};

/// Stable validation output suitable for Studio's issue list and CLI tools.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MorphDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
}

impl MorphDiagnostic {
    pub(crate) fn error(code: &str, path: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            path: path.to_owned(),
            message: message.into(),
        }
    }
}
