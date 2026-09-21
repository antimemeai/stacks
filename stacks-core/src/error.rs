use thiserror::Error;

/// Failure at the model boundary (bad token, inconsistent value).
#[derive(Debug, Error)]
pub enum ModelError {
    /// A string that is not a valid token for the named closed enum.
    #[error("unknown {type_name} token: {token:?}")]
    UnknownToken {
        type_name: &'static str,
        token: String,
    },
    /// A range operator whose bounds are inverted.
    #[error("range operator requires min <= max, got {min} > {max}")]
    InvertedRange { min: f64, max: f64 },
}

/// Failure at the storage boundary.
#[derive(Debug, Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error("migration {version} failed: {reason}")]
    Migration { version: i64, reason: String },
    /// A row read back from the DB violates the model (should be impossible
    /// while CHECK constraints hold; surfaced rather than silently coerced).
    #[error("row failed model validation: {0}")]
    CorruptRow(String),
}
