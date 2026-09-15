//! The permissive version probe.
//!
//! Decoding reads the version through this structure *before* attempting the
//! full session, so a recording from a future build is reported as an
//! unsupported version rather than as a confusing complaint about an unknown
//! field. It therefore must **not** carry `deny_unknown_fields`: ignoring
//! everything it does not recognize is its whole job.
//!
//! Two nested probes rather than one, because the version sits inside the
//! header: a flat probe would have to know the header's field names, which is
//! exactly the coupling the probe exists to avoid.

use serde::Deserialize;

use super::format::FormatVersion;

/// A deliberately permissive view of a session's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(super) struct HeaderProbe {
    /// The declared schema version.
    pub(super) version: FormatVersion,
}

/// A deliberately permissive view of a recorded session's version field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(super) struct SessionProbe {
    /// The session's header, read for its version alone.
    pub(super) header: HeaderProbe,
}
