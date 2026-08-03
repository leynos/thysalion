//! Encoding and decoding for the scene document.
//!
//! Two encodings, one structure: JSON for authoring and diffing, MessagePack
//! for shipping. Decoding always probes the version first, then deserializes.
//!
//! MessagePack is written with `rmp_serde::to_vec_named` and never `to_vec`.
//! That is a *correctness* requirement rather than a preference for
//! evolvability: `rmp_serde` writes structs as positional arrays by default,
//! its decoder accepts either shape silently, and `deny_unknown_fields` has no
//! field names to act on in the array form. An accidental `to_vec` would
//! therefore ship a scene that is positional, unevolvable, and rejects
//! nothing, while looking healthy to any round-trip test. Only the wire-shape
//! assertion in `tests/document_round_trip.rs` catches it.

mod json;
mod msgpack;

use camino::Utf8Path;
use smol_str::SmolStr;

use crate::scene::document::{SUPPORTED_VERSION, SceneDocument};

/// Which on-disk form a scene document takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Encoding {
    /// The authoring and diffing form.
    Json,
    /// The shipping form.
    MessagePack,
}

impl Encoding {
    /// The conventional file extension for this encoding.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::MessagePack => "msgpack",
        }
    }

    /// Infers the encoding from a path's extension.
    ///
    /// Returns `None` for an unrecognized or absent extension; the caller
    /// reports that rather than guessing, because guessing wrong produces a
    /// parse error that blames the document for the caller's mistake.
    #[must_use]
    pub fn from_path(path: &Utf8Path) -> Option<Self> {
        match path.extension() {
            Some("json") => Some(Self::Json),
            Some("msgpack") => Some(Self::MessagePack),
            _ => None,
        }
    }
}

impl core::fmt::Display for Encoding {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Json => "JSON",
            Self::MessagePack => "MessagePack",
        })
    }
}

/// Why a document could not be decoded or encoded.
///
/// Distinct from a validation diagnostic: a malformed document yields exactly
/// one of these and *no* document, so nothing can accumulate with it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CodecError {
    /// The bytes are not well-formed in the stated encoding.
    #[error("malformed {encoding} document at {pointer}: {message}")]
    Malformed {
        /// The encoding the bytes were read as.
        encoding: Encoding,
        /// Structural path to the offending value, such as
        /// `palette[2].emission.intensity`.
        pointer: String,
        /// The underlying parser's message.
        message: SmolStr,
    },
    /// The document declares a version this build cannot read.
    #[error("unsupported document version {found}; this build supports {supported}")]
    UnsupportedVersion {
        /// The version the document declares.
        found: crate::scene::document::DocumentVersion,
        /// The version this build writes and understands.
        supported: crate::scene::document::DocumentVersion,
    },
    /// The document could not be encoded.
    #[error("cannot encode {encoding} document: {message}")]
    Encode {
        /// The encoding that failed.
        encoding: Encoding,
        /// The underlying encoder's message.
        message: SmolStr,
    },
}

/// Encodes a document in the given encoding.
///
/// JSON is written compactly. Pretty-printing was measured at 3.1 times the
/// size for the busiest plausible fixture — over the fixture-size budget — and
/// it buys a diff nobody reads, because generated fixtures are not the review
/// surface: the layered-text authoring sources are.
///
/// # Errors
///
/// Returns [`CodecError::Encode`] when the underlying encoder fails.
pub fn encode_document(
    document: &SceneDocument,
    encoding: Encoding,
) -> Result<Vec<u8>, CodecError> {
    match encoding {
        Encoding::Json => json::encode(document),
        Encoding::MessagePack => msgpack::encode(document),
    }
}

/// Decodes a document, probing its version before deserializing it.
///
/// # Errors
///
/// Returns [`CodecError::UnsupportedVersion`] when the document declares a
/// version outwith this build's range, and [`CodecError::Malformed`] when the
/// bytes do not parse.
pub fn decode_document(bytes: &[u8], encoding: Encoding) -> Result<SceneDocument, CodecError> {
    let probed = match encoding {
        Encoding::Json => json::probe_version(bytes),
        Encoding::MessagePack => msgpack::probe_version(bytes),
    }?;
    if !SUPPORTED_VERSION.accepts(probed) {
        return Err(CodecError::UnsupportedVersion {
            found: probed,
            supported: SUPPORTED_VERSION,
        });
    }
    match encoding {
        Encoding::Json => json::decode(bytes),
        Encoding::MessagePack => msgpack::decode(bytes),
    }
}
