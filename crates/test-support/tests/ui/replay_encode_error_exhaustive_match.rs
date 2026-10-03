use thysalion_test_support::replay::ReplayEncodeError;

fn error_name(error: ReplayEncodeError) -> &'static str {
    match error {
        ReplayEncodeError::Encode { .. } => "encode",
        ReplayEncodeError::NonMonotonicTick { .. } => "tick order",
        ReplayEncodeError::UnsupportedVersion { .. } => "version",
    }
}

fn main() {
    let _ = error_name as fn(ReplayEncodeError) -> &'static str;
}
