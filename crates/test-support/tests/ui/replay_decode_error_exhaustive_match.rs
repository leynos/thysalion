use thysalion_test_support::replay::ReplayDecodeError;

fn error_name(error: ReplayDecodeError) -> &'static str {
    match error {
        ReplayDecodeError::Malformed { .. } => "malformed",
        ReplayDecodeError::NonMonotonicTick { .. } => "tick order",
        ReplayDecodeError::UnsupportedVersion { .. } => "version",
    }
}

fn main() {
    let _ = error_name as fn(ReplayDecodeError) -> &'static str;
}
