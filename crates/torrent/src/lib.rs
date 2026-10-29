//! Getting video over BitTorrent.
//!
//! - [`downloads`]: search the admin's Torznab indexers (Jackett, Prowlarr),
//!   download a release with the built-in torrent client, and file it into a
//!   library when it's done.
//! - [`streams`]: play a release while it downloads, fetching the part being
//!   watched first; nothing is kept afterwards.
//!
//! One torrent session serves both. The HTTP handlers live in the server
//! crate; this crate knows nothing of routes or accounts.

pub mod downloads;
pub mod streams;
mod whole_pieces;
