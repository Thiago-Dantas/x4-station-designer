//! Core library of the X4 station designer.
//!
//! See `docs/x4-construction-plans.md` for the file format and snap math this
//! crate implements, and `docs/x4-game-data.md` for the game files it reads.

pub mod catalog;
pub mod design;
pub mod game;
pub mod geometry;
pub mod import;
pub mod mesh;
pub mod plan;
pub mod plot;
pub mod text;
pub mod texture;
pub mod validate;
pub mod vfs;

use std::fmt;

/// Error for everything in this crate: a message with enough context to act on.
#[derive(Debug, Clone, PartialEq)]
pub struct Error(pub String);

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error(e.to_string())
    }
}

impl From<roxmltree::Error> for Error {
    fn from(e: roxmltree::Error) -> Self {
        Error(e.to_string())
    }
}
