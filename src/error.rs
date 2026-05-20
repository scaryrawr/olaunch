use thiserror::Error;

#[derive(Debug, Error)]
pub enum OlaunchError {
    #[error("{0}")]
    Message(String),
    #[error(
        "unknown integration `{name}`\n\nRun `olaunch list integrations` to see supported integrations."
    )]
    UnknownIntegration { name: String },
    #[error("unknown provider `{name}`\n\nSupported providers: lmstudio, ollama, omlx, generic.")]
    UnknownProvider { name: String },
    #[error(
        "no --model provided for non-interactive launch; pass --model <model> or run from an interactive terminal"
    )]
    MissingModelNonInteractive,
    #[error("{program} is not installed\n\n{hint}")]
    MissingProgram { program: String, hint: String },
    #[error("no models discovered from local providers: {0}")]
    NoModelsDiscovered(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Yaml(#[from] yaml_serde::Error),
    #[error(transparent)]
    Toml(#[from] toml_edit::TomlError),
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, OlaunchError>;

impl OlaunchError {
    pub fn exit_code(&self) -> i32 {
        match self {
            OlaunchError::MissingModelNonInteractive => 2,
            OlaunchError::UnknownIntegration { .. } | OlaunchError::UnknownProvider { .. } => 64,
            OlaunchError::MissingProgram { .. } => 69,
            _ => 1,
        }
    }
}
