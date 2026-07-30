//! The MessagePack encoding: the shipping form.
//!
//! Always written with `to_vec_named`. See the module comment on
//! [`super`] for why that is a correctness requirement rather than a
//! preference, and `tests/document_round_trip.rs` for the assertion that
//! enforces it.
//!
//! `serde_path_to_error` works here too, which the 1.2 execution plan
//! expected it might not: a malformed payload yields the same structural path
//! as the JSON decoder produces for the same fault. MessagePack carries no
//! line or column, which is inherent to a binary format.

use serde::de::DeserializeOwned;
use smol_str::SmolStr;

use super::{CodecError, Encoding};
use crate::scene::document::{DocumentVersion, SceneDocument, VersionProbe};

pub(super) fn encode(document: &SceneDocument) -> Result<Vec<u8>, CodecError> {
    rmp_serde::to_vec_named(document).map_err(|error| CodecError::Encode {
        encoding: Encoding::MessagePack,
        message: SmolStr::new(error.to_string()),
    })
}

/// Deserializes `T`, attaching a structural path to any failure.
fn located<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError> {
    let mut deserializer = rmp_serde::Deserializer::new(bytes);
    serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        let pointer = error.path().to_string();
        CodecError::Malformed {
            encoding: Encoding::MessagePack,
            pointer,
            message: SmolStr::new(error.into_inner().to_string()),
        }
    })
}

pub(super) fn probe_version(bytes: &[u8]) -> Result<DocumentVersion, CodecError> {
    located::<VersionProbe>(bytes).map(|probe| probe.version)
}

pub(super) fn decode(bytes: &[u8]) -> Result<SceneDocument, CodecError> { located(bytes) }
