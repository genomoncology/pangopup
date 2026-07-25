//! Shared bounded asset failure vocabulary.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetErrorKind {
    InputIo,
    OutputIo,
    ManifestInvalid,
    TransportIncompatible,
    PartSetInvalid,
    TransportHashMismatch,
    CompressionInvalid,
    BundleInvalid,
    OutputConflict,
    UnsupportedPlatform,
    PathInvalid,
    PathUnavailable,
    AssetLocked,
    AssetIo,
    AssetStateInvalid,
    StagingInvalid,
    InstallConflict,
    AssetsMissing,
    ReleaseInvalid,
    ReleaseUpload,
    AssetDownload,
    AssetTimeout,
}

impl AssetErrorKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::InputIo => "INPUT_IO",
            Self::OutputIo => "OUTPUT_IO",
            Self::ManifestInvalid => "MANIFEST_INVALID",
            Self::TransportIncompatible => "TRANSPORT_INCOMPATIBLE",
            Self::PartSetInvalid => "PART_SET_INVALID",
            Self::TransportHashMismatch => "TRANSPORT_HASH_MISMATCH",
            Self::CompressionInvalid => "COMPRESSION_INVALID",
            Self::BundleInvalid => "BUNDLE_INVALID",
            Self::OutputConflict => "OUTPUT_CONFLICT",
            Self::UnsupportedPlatform => "UNSUPPORTED_PLATFORM",
            Self::PathInvalid => "PATH_INVALID",
            Self::PathUnavailable => "PATH_UNAVAILABLE",
            Self::AssetLocked => "ASSET_LOCKED",
            Self::AssetIo => "ASSET_IO",
            Self::AssetStateInvalid => "ASSET_STATE_INVALID",
            Self::StagingInvalid => "STAGING_INVALID",
            Self::InstallConflict => "INSTALL_CONFLICT",
            Self::AssetsMissing => "ASSETS_MISSING",
            Self::ReleaseInvalid => "RELEASE_INVALID",
            Self::ReleaseUpload => "RELEASE_UPLOAD",
            Self::AssetDownload => "ASSET_DOWNLOAD",
            Self::AssetTimeout => "ASSET_TIMEOUT",
        }
    }
}

#[derive(Debug)]
pub struct AssetError {
    pub(crate) kind: AssetErrorKind,
    pub(crate) legacy_code: Option<&'static str>,
    pub(crate) message: String,
}

impl AssetError {
    pub fn new(kind: AssetErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            legacy_code: None,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> AssetErrorKind {
        self.kind
    }

    pub fn legacy_build_code(&self) -> Option<&'static str> {
        self.legacy_code
    }
}

impl fmt::Display for AssetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AssetError {}
