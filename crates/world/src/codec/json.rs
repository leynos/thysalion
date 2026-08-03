//! The JSON encoding: the authoring and diffing form.
//!
//! Decoding runs through `serde_path_to_error`, which turns serde's
//! "invalid type: string" into "`palette[2].emission.intensity`: invalid type:
//! string". Without it the reader is told what went wrong but not where, which
//! in a document with thousands of runs is most of the problem.

use serde::de::DeserializeOwned;
use smol_str::SmolStr;

use super::{CodecError, Encoding};
use crate::scene::document::{DocumentVersion, SceneDocument, VersionProbe};

/// Encodes compactly. See [`super::encode_document`] for why not pretty.
pub(super) fn encode(document: &SceneDocument) -> Result<Vec<u8>, CodecError> {
    serde_json::to_vec(document).map_err(|error| CodecError::Encode {
        encoding: Encoding::Json,
        message: SmolStr::new(error.to_string()),
    })
}

/// Deserializes `T`, attaching a structural path to any failure.
fn located<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        let pointer = error.path().to_string();
        CodecError::Malformed {
            encoding: Encoding::Json,
            pointer,
            message: SmolStr::new(error.into_inner().to_string()),
        }
    })?;
    // A document is one value, not a prefix of a stream. Without this, bytes
    // after the closing brace are discarded in silence, so a file holding two
    // scenes — or one scene and the tail of an interrupted write — loads as
    // whichever came first and reports nothing.
    deserializer.end().map_err(|error| CodecError::Malformed {
        encoding: Encoding::Json,
        pointer: String::from("/"),
        message: SmolStr::new(format!("trailing input after the document: {error}")),
    })?;
    Ok(value)
}

pub(super) fn probe_version(bytes: &[u8]) -> Result<DocumentVersion, CodecError> {
    located::<VersionProbe>(bytes).map(|probe| probe.version)
}

pub(super) fn decode(bytes: &[u8]) -> Result<SceneDocument, CodecError> { located(bytes) }
