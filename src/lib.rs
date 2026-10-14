#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

pub mod budget;
pub mod cli;
pub mod compositor;
pub mod converter;
pub mod encode;
pub mod error;
pub mod format;
pub mod frame;
pub mod gif;
pub mod input;
pub mod options;
pub mod progress;
pub mod quantise;
pub mod squish;
pub mod video;
pub mod vtf;
