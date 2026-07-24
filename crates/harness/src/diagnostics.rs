//! Published diagnostic paths for the harness overlay.
//!
//! Diagnostics flow through Bevy's `DiagnosticsStore` under typed
//! [`DiagnosticPath`] constants — no bespoke stringly-named counter
//! registry. This is a debug heads-up display, deliberately not the
//! `metrics` crate: there is no exporter and no label-cardinality
//! concern for an on-screen overlay.
//!
//! [`TICK_TIME`] is the cross-plane seam defined by the 1.1 execplan: the
//! simulation plane (roadmap phase 4) measures its tick and writes the
//! value through the composition root; the overlay displays it whenever a
//! measurement is present. Design §10.6's per-operator trace counters
//! later register additional paths the same way, without changing the
//! overlay.

use bevy::diagnostic::DiagnosticPath;

/// Frame-time diagnostic path (Bevy's own, re-exported for one-stop use).
pub const FRAME_TIME: DiagnosticPath = bevy::diagnostic::FrameTimeDiagnosticsPlugin::FRAME_TIME;

/// Frames-per-second diagnostic path (Bevy's own).
pub const FPS: DiagnosticPath = bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS;

/// Simulation tick time in milliseconds. Registered by
/// `HarnessCorePlugin`; written by the simulation plane from phase 4.
pub const TICK_TIME: DiagnosticPath = DiagnosticPath::const_new("thysalion/tick_time");
