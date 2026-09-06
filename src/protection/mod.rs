//! Optional M5 file protection.
//!
//! The public surface deliberately exposes one closed suite. Algorithm and
//! entropy selection are not configurable.

mod codec;
mod crypto;
#[cfg(all(test, windows))]
mod m6_tests;
mod operations;
mod publication;
mod stream;
mod windows;

pub use operations::{generate_keypair, protect_file, unprotect_file};

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::path::PathBuf;

/// Result of an atomic no-clobber publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicationStatus {
    /// The final name was committed and the private staging name was removed.
    Published,
    /// The final name was committed, but cleanup of this staging name failed.
    PublishedWithStagingResidue(PathBuf),
}

/// A non-fatal, structured warning produced by a best-effort mitigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtectionWarning {
    /// Restricting a directory or secret-key DACL did not succeed.
    PermissionRestrictionFailed { path: PathBuf, reason: String },
}

impl Display for ProtectionWarning {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionRestrictionFailed { path, reason } => write!(
                f,
                "could not restrict permissions for {}: {reason}",
                path.display()
            ),
        }
    }
}

/// Successful result of one protected or unprotected file publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectionOutcome {
    pub publication: PublicationStatus,
    pub warnings: Vec<ProtectionWarning>,
}

/// Successful result of creating the fixed `public.key` and `secret.key` pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyGenerationOutcome {
    pub public_key: PublicationStatus,
    pub secret_key: PublicationStatus,
    pub warnings: Vec<ProtectionWarning>,
}

/// Typed M5 failure. Cryptographic failures have deliberately uniform text.
#[derive(Debug)]
pub enum ProtectionError {
    UnsupportedPublicationPlatform {
        path: PathBuf,
        reason: &'static str,
    },
    OutputAlreadyExists {
        path: PathBuf,
    },
    DestinationInM4Namespace {
        path: PathBuf,
        root: PathBuf,
    },
    UnsafePath {
        path: PathBuf,
        reason: &'static str,
    },
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    EntropyUnavailable {
        source: getrandom::Error,
    },
    InvalidMagic {
        artifact: &'static str,
    },
    UnsupportedVersion {
        artifact: &'static str,
        version: u16,
    },
    UnsupportedAlgorithm {
        algorithm: u16,
    },
    UnsupportedSuite {
        suite: u16,
    },
    InvalidKey {
        path: PathBuf,
        reason: &'static str,
    },
    RecipientFingerprintMismatch,
    InputTooLarge {
        length: u64,
        maximum: u64,
    },
    InvalidLength {
        artifact: &'static str,
        expected: u64,
        actual: u64,
    },
    ArithmeticOverflow {
        operation: &'static str,
    },
    AuthenticationFailed,
    InvalidFrameSequence {
        reason: &'static str,
    },
}

impl Display for ProtectionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPublicationPlatform { path, reason } => write!(
                f,
                "unsupported M5 publication target {}: {reason}",
                path.display()
            ),
            Self::OutputAlreadyExists { path } => {
                write!(f, "M5 output already exists: {}", path.display())
            }
            Self::DestinationInM4Namespace { path, root } => write!(
                f,
                "M5 destination {} is inside M4-managed namespace {}",
                path.display(),
                root.display()
            ),
            Self::UnsafePath { path, reason } => {
                write!(f, "unsafe M5 output path {}: {reason}", path.display())
            }
            Self::Io {
                operation,
                path,
                source,
            } => write!(f, "{operation} {}: {source}", path.display()),
            Self::EntropyUnavailable { source } => {
                write!(f, "operating-system entropy unavailable: {source}")
            }
            Self::InvalidMagic { artifact } => write!(f, "invalid {artifact} magic"),
            Self::UnsupportedVersion { artifact, version } => {
                write!(f, "unsupported {artifact} version {version}")
            }
            Self::UnsupportedAlgorithm { algorithm } => {
                write!(f, "unsupported M5 key algorithm {algorithm}")
            }
            Self::UnsupportedSuite { suite } => {
                write!(f, "unsupported M5 envelope suite {suite}")
            }
            Self::InvalidKey { path, reason } => {
                write!(f, "invalid M5 key {}: {reason}", path.display())
            }
            // Keep wrong key, invalid encapsulation and an invalid tag externally
            // indistinguishable in human-facing diagnostics.
            Self::RecipientFingerprintMismatch | Self::AuthenticationFailed => {
                f.write_str("M5 authentication/unprotection failed")
            }
            Self::InputTooLarge { length, maximum } => write!(
                f,
                "M5 input has {length} bytes, exceeding the {maximum}-byte limit"
            ),
            Self::InvalidLength {
                artifact,
                expected,
                actual,
            } => write!(
                f,
                "invalid {artifact} length: expected {expected} bytes, got {actual}"
            ),
            Self::ArithmeticOverflow { operation } => {
                write!(f, "integer overflow while computing {operation}")
            }
            Self::InvalidFrameSequence { reason } => {
                write!(f, "invalid M5 frame sequence: {reason}")
            }
        }
    }
}

impl Error for ProtectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::EntropyUnavailable { source } => Some(source),
            _ => None,
        }
    }
}

pub(crate) fn io_error(
    operation: &'static str,
    path: &std::path::Path,
    source: io::Error,
) -> ProtectionError {
    ProtectionError::Io {
        operation,
        path: path.to_owned(),
        source,
    }
}
