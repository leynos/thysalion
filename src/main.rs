//! `Thysalion` application entry point.

use std::io::{self, Write};

/// Application entry point.
fn main() -> io::Result<()> { writeln!(io::stdout().lock(), "{}", thysalion::greet()) }
