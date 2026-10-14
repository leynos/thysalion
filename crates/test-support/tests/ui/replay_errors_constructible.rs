//! Compile-time contract: external callers can construct replay error values.

use thysalion_test_support::replay::{
    FormatVersion, ReplayDecodeError, ReplayEncodeError,
};

fn main() {
    let version = FormatVersion { major: 1, minor: 0 };
    let _encode = ReplayEncodeError::UnsupportedVersion {
        found: version,
        supported: version,
    };
    let _decode = ReplayDecodeError::UnsupportedVersion {
        found: version,
        supported: version,
    };
}
