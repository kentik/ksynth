#![allow(clippy::redundant_field_names)]

pub use artifact::Artifact;
pub use artifact::Arch;
pub use artifact::System;
pub use artifact::Target;

pub use client::Client;
pub use client::Item;
pub use client::Query;

pub use error::Error;

pub use update::Update;
pub use update::Updates;

mod artifact;
mod client;
mod error;
mod expand;
mod time;
mod update;
