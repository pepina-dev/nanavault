//! The cryptographic core: key derivation, file encryption, and Shamir secret
//! sharing. Nothing in here performs I/O — it is pure, deterministic, and
//! unit-testable in isolation.

pub mod cipher;
pub mod kdf;
pub mod slip39;
