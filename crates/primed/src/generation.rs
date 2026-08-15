use prime_contracts::{GenerationRecord, GENERATION_SCHEMA};
use std::fs;
use std::io;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GenerationError {
    #[error("generation I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("generation JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("generation schema is {found}, expected {expected}")]
    Schema { found: String, expected: &'static str },
    #[error("generation image digest must be immutable sha256")]
    ImageDigest,
    #[error("generation source revision is empty")]
    SourceRevision,
}

pub fn load(path: &Path) -> Result<GenerationRecord, GenerationError> {
    let bytes = fs::read(path)?;
    let generation: GenerationRecord = serde_json::from_slice(&bytes)?;
    if generation.schema != GENERATION_SCHEMA {
        return Err(GenerationError::Schema {
            found: generation.schema,
            expected: GENERATION_SCHEMA,
        });
    }
    if !generation.image_digest.starts_with("sha256:") || generation.image_digest.len() <= 7 {
        return Err(GenerationError::ImageDigest);
    }
    if generation.source_revision.trim().is_empty() {
        return Err(GenerationError::SourceRevision);
    }
    Ok(generation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_generation_is_not_invented() {
        let dir = tempfile::tempdir().expect("tempdir");
        let error = load(&dir.path().join("missing.json")).expect_err("must fail");
        assert!(matches!(error, GenerationError::Io(_)));
    }
}
