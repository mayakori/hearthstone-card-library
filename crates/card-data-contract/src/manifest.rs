use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    normalized::{is_lowercase_sha256, CardCounts},
    version::DataVersion,
    MINIMUM_APP_VERSION, SCHEMA_VERSION, SUPPORTED_LOCALES,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManifestValidationError {
    #[error("schemaVersion must be {SCHEMA_VERSION}")]
    SchemaVersion,
    #[error("minimumAppVersion must be {MINIMUM_APP_VERSION}")]
    MinimumAppVersion,
    #[error("dataVersion is invalid: {0}")]
    DataVersion(String),
    #[error("official version fields do not match dataVersion")]
    VersionParts,
    #[error("supportedLocales must be [ko_KR, en_US]")]
    SupportedLocales,
    #[error("locale keys must exactly equal supportedLocales")]
    LocaleKeys,
    #[error("locale {0} has invalid card counts")]
    CardCounts(String),
    #[error("locale {locale} has an invalid SHA-256 in {asset}")]
    Sha256 { locale: String, asset: &'static str },
    #[error("locale {locale} has an invalid defaultDownload in {asset}")]
    DefaultDownload { locale: String, asset: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema_version: u32,
    pub minimum_app_version: String,
    pub data_version: String,
    pub official_patch_version: String,
    pub build_id: u64,
    pub revision: u64,
    pub generated_at: String,
    pub supported_locales: Vec<String>,
    pub locales: BTreeMap<String, LocaleManifest>,
}

impl Manifest {
    pub fn validate(&self) -> Result<(), ManifestValidationError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ManifestValidationError::SchemaVersion);
        }
        if self.minimum_app_version != MINIMUM_APP_VERSION {
            return Err(ManifestValidationError::MinimumAppVersion);
        }
        let data_version = DataVersion::parse(&self.data_version)
            .map_err(|_| ManifestValidationError::DataVersion(self.data_version.clone()))?;
        if self.official_patch_version != data_version.official_patch_version()
            || self.build_id != data_version.build_id()
            || self.revision != data_version.revision()
        {
            return Err(ManifestValidationError::VersionParts);
        }
        let expected = SUPPORTED_LOCALES.map(String::from).to_vec();
        if self.supported_locales != expected {
            return Err(ManifestValidationError::SupportedLocales);
        }
        let locales = self.locales.keys().cloned().collect::<BTreeSet<_>>();
        let supported = self
            .supported_locales
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if locales != supported {
            return Err(ManifestValidationError::LocaleKeys);
        }
        for (locale, assets) in &self.locales {
            if !assets.card_counts.is_valid() {
                return Err(ManifestValidationError::CardCounts(locale.clone()));
            }
            assets.raw.validate(locale, "raw", false)?;
            assets.normalized.validate(locale, "normalized", true)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleManifest {
    pub card_counts: CardCounts,
    pub raw: AssetDescriptor,
    pub normalized: AssetDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDescriptor {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub compression: String,
    pub uncompressed_bytes: u64,
    pub uncompressed_sha256: String,
    pub default_download: bool,
}

impl AssetDescriptor {
    fn validate(
        &self,
        locale: &str,
        asset: &'static str,
        expected_default_download: bool,
    ) -> Result<(), ManifestValidationError> {
        if !is_lowercase_sha256(&self.sha256) || !is_lowercase_sha256(&self.uncompressed_sha256) {
            return Err(ManifestValidationError::Sha256 {
                locale: locale.into(),
                asset,
            });
        }
        if self.default_download != expected_default_download {
            return Err(ManifestValidationError::DefaultDownload {
                locale: locale.into(),
                asset,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{AssetDescriptor, LocaleManifest, Manifest};
    use crate::{normalized::CardCounts, MINIMUM_APP_VERSION, SCHEMA_VERSION};

    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn asset(default_download: bool) -> AssetDescriptor {
        AssetDescriptor {
            path: "cards.zst".into(),
            bytes: 1,
            sha256: HASH.into(),
            compression: "zstd".into(),
            uncompressed_bytes: 1,
            uncompressed_sha256: HASH.into(),
            default_download,
        }
    }

    fn manifest() -> Manifest {
        let locale = LocaleManifest {
            card_counts: CardCounts {
                standard: 1,
                related: 2,
                class_reference: 3,
                total: 6,
            },
            raw: asset(false),
            normalized: asset(true),
        };
        Manifest {
            schema_version: SCHEMA_VERSION,
            minimum_app_version: MINIMUM_APP_VERSION.into(),
            data_version: "36.0.3-build247416-r1".into(),
            official_patch_version: "36.0.3".into(),
            build_id: 247_416,
            revision: 1,
            generated_at: "2026-08-05T00:00:00Z".into(),
            supported_locales: vec!["ko_KR".into(), "en_US".into()],
            locales: BTreeMap::from([("ko_KR".into(), locale.clone()), ("en_US".into(), locale)]),
        }
    }

    #[test]
    fn manifest_validates_the_fixed_locale_assets_and_counts() {
        manifest().validate().unwrap();
    }

    #[test]
    fn manifest_rejects_noncanonical_hash_and_download_defaults() {
        let mut invalid_manifest = manifest();
        invalid_manifest
            .locales
            .get_mut("ko_KR")
            .unwrap()
            .raw
            .sha256 = "ABC".into();
        assert!(invalid_manifest.validate().is_err());

        let mut manifest = manifest();
        manifest
            .locales
            .get_mut("ko_KR")
            .unwrap()
            .normalized
            .default_download = false;
        assert!(manifest.validate().is_err());
    }
}
