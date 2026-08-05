pub mod manifest;
pub mod normalized;
pub mod official;
pub mod raw;
pub mod text;
pub mod version;

pub use manifest::{AssetDescriptor, LocaleManifest, Manifest, ManifestValidationError};
pub use normalized::{
    CardCounts, CardRelation, NormalizedCatalog, NormalizedContractError, RelationKind, SourceField,
};
pub use official::{CardsPageResponse, MetadataResponse, OfficialCard};
pub use raw::{canonical_json_bytes, RawContractError, RawSnapshot};
pub use text::plain_text;
pub use version::DataVersion;

pub const SCHEMA_VERSION: u32 = 1;
pub const MINIMUM_APP_VERSION: &str = "0.1.0";
pub const SUPPORTED_LOCALES: [&str; 2] = ["ko_KR", "en_US"];

#[cfg(test)]
mod contract_tests {
    use serde_json::json;

    use crate::{raw::canonical_json_bytes, text::plain_text};

    #[test]
    fn canonical_json_sorts_response_objects_but_preserves_arrays() {
        let value = json!({"z": 1, "a": {"y": 2, "b": 3}, "items": [2, 1]});
        assert_eq!(
            canonical_json_bytes(&value).unwrap(),
            b"{\"a\":{\"b\":3,\"y\":2},\"items\":[2,1],\"z\":1}\n",
        );
    }

    #[test]
    fn derives_plain_text_without_rewriting_inner_whitespace() {
        let markup = "<b>전투의 함성:</b><br>피해를&nbsp; 2 줍니다.";
        assert_eq!(plain_text(markup), "전투의 함성:\n피해를\u{a0} 2 줍니다.");
    }
}
