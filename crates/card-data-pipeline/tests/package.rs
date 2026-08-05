use std::{collections::BTreeMap, fs};

use card_data_contract::{CardCounts, DataVersion};
use card_data_pipeline::{
    validate_package_directory, PackageBuilder, PackageLocaleInput, PackageRequest, PipelineError,
};

const DATA_VERSION: &str = "36.0.3-build247416-r1";
const GENERATED_AT: &str = "2026-08-05T00:00:00Z";

fn request(root: &std::path::Path) -> PackageRequest {
    request_with_input(root, "input")
}

fn request_with_input(root: &std::path::Path, input_name: &str) -> PackageRequest {
    let input = root.join(input_name);
    fs::create_dir_all(&input).unwrap();
    let mut locales = BTreeMap::new();
    for locale in ["ko_KR", "en_US"] {
        let sqlite = input.join(format!("{locale}.sqlite"));
        let raw_bytes = format!("{{\"locale\":\"{locale}\"}}\n").into_bytes();
        write_sqlite(&sqlite, locale, &raw_bytes);
        locales.insert(
            locale.to_owned(),
            PackageLocaleInput {
                raw_bytes,
                sqlite_path: sqlite,
                card_counts: CardCounts {
                    standard: 7,
                    related: 3,
                    class_reference: 2,
                    total: 12,
                },
            },
        );
    }
    PackageRequest {
        data_version: DataVersion::parse(DATA_VERSION).unwrap(),
        output_root: root.join("output"),
        generated_at: GENERATED_AT.into(),
        locales,
    }
}

fn write_sqlite(path: &std::path::Path, locale: &str, raw: &[u8]) {
    use sha2::{Digest, Sha256};

    let connection = rusqlite::Connection::open(path).unwrap();
    connection.execute_batch("PRAGMA user_version = 1; CREATE TABLE catalog_metadata (schema_version INTEGER, data_version TEXT, locale TEXT, generated_at TEXT, source_raw_sha256 TEXT, standard_card_count INTEGER, related_card_count INTEGER, class_reference_card_count INTEGER, total_card_count INTEGER);").unwrap();
    connection
        .execute(
            "INSERT INTO catalog_metadata VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                1,
                DATA_VERSION,
                locale,
                GENERATED_AT,
                hex::encode(Sha256::digest(raw)),
                7,
                3,
                2,
                12,
            ],
        )
        .unwrap();
}

#[test]
fn builds_byte_identical_packages_under_two_output_roots() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let first_result = PackageBuilder::build(request(first.path())).unwrap();
    let second_result = PackageBuilder::build(request(second.path())).unwrap();

    for path in [
        "raw/ko_KR.json.zst",
        "raw/en_US.json.zst",
        "normalized/ko_KR.sqlite.zst",
        "normalized/en_US.sqlite.zst",
        "manifest.json",
    ] {
        assert_eq!(
            fs::read(first_result.version_directory.join(path)).unwrap(),
            fs::read(second_result.version_directory.join(path)).unwrap(),
            "{path}",
        );
    }
    validate_package_directory(&first_result.version_directory, &first_result.manifest).unwrap();
    assert_eq!(
        decompress(&first_result.version_directory.join("raw/ko_KR.json.zst")),
        decompress(&second_result.version_directory.join("raw/ko_KR.json.zst"))
    );
    assert_eq!(
        decompress(&first_result.version_directory.join("raw/en_US.json.zst")),
        decompress(&second_result.version_directory.join("raw/en_US.json.zst"))
    );
    assert_eq!(
        decompress(
            &first_result
                .version_directory
                .join("normalized/ko_KR.sqlite.zst")
        ),
        decompress(
            &second_result
                .version_directory
                .join("normalized/ko_KR.sqlite.zst")
        )
    );
    assert_eq!(
        decompress(
            &first_result
                .version_directory
                .join("normalized/en_US.sqlite.zst")
        ),
        decompress(
            &second_result
                .version_directory
                .join("normalized/en_US.sqlite.zst")
        )
    );
    let files = walk(
        &first_result.version_directory,
        &first_result.version_directory,
    );
    assert_eq!(
        files,
        [
            "manifest.json",
            "normalized/en_US.sqlite.zst",
            "normalized/ko_KR.sqlite.zst",
            "raw/en_US.json.zst",
            "raw/ko_KR.json.zst",
        ]
    );
    for path in [
        "raw/ko_KR.json.zst",
        "raw/en_US.json.zst",
        "normalized/ko_KR.sqlite.zst",
        "normalized/en_US.sqlite.zst",
    ] {
        let frame = fs::read(first_result.version_directory.join(path)).unwrap();
        assert_eq!(&frame[..4], &[0x28, 0xb5, 0x2f, 0xfd], "{path}");
        assert_ne!(frame[4] & 0b0000_0100, 0, "{path} has checksum");
        assert!(
            zstd::zstd_safe::get_frame_content_size(&frame)
                .unwrap()
                .is_some(),
            "{path} records content size"
        );
        assert_eq!(
            zstd::zstd_safe::find_frame_compressed_size(&frame).unwrap(),
            frame.len(),
            "{path} is one frame"
        );
    }
}

#[test]
fn rejects_tampered_assets_and_leaves_existing_version_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let result = PackageBuilder::build(request(root.path())).unwrap();
    let raw = result.version_directory.join("raw/ko_KR.json.zst");
    fs::write(&raw, b"tampered").unwrap();
    assert!(validate_package_directory(&result.version_directory, &result.manifest).is_err());
    fs::write(&raw, b"still tampered").unwrap();
    fs::create_dir(result.version_directory.join("unexpected-empty-directory")).unwrap();
    assert!(validate_package_directory(&result.version_directory, &result.manifest).is_err());

    let existing = result.version_directory.join("manifest.json");
    let before = fs::read(&existing).unwrap();
    assert!(matches!(
        PackageBuilder::build(request_with_input(root.path(), "collision-input")),
        Err(PipelineError::Config(_))
    ));
    assert_eq!(fs::read(existing).unwrap(), before);
}

#[test]
fn rejects_noncanonical_manifest_bytes_and_missing_assets() {
    let root = tempfile::tempdir().unwrap();
    let result = PackageBuilder::build(request(root.path())).unwrap();
    let path = result.version_directory.join("manifest.json");
    let canonical = fs::read(&path).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
    fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    assert!(validate_package_directory(&result.version_directory, &result.manifest).is_err());

    let mut reordered = serde_json::to_vec(&value).unwrap();
    reordered.push(b'\n');
    assert_ne!(reordered, canonical);
    fs::write(&path, reordered).unwrap();
    assert!(validate_package_directory(&result.version_directory, &result.manifest).is_err());

    let mut duplicate_key = canonical.clone();
    duplicate_key.splice(1..1, b"\"schemaVersion\":999,".iter().copied());
    fs::write(&path, duplicate_key).unwrap();
    assert!(validate_package_directory(&result.version_directory, &result.manifest).is_err());

    fs::write(&path, canonical).unwrap();
    fs::remove_file(result.version_directory.join("raw/en_US.json.zst")).unwrap();
    assert!(validate_package_directory(&result.version_directory, &result.manifest).is_err());
}

#[test]
fn rejects_sqlite_metadata_that_does_not_match_the_package_inputs() {
    let root = tempfile::tempdir().unwrap();
    let request = request(root.path());
    let sqlite = request.locales["ko_KR"].sqlite_path.clone();
    rusqlite::Connection::open(sqlite)
        .unwrap()
        .execute(
            "UPDATE catalog_metadata SET generated_at = '2026-08-05T00:00:01Z'",
            [],
        )
        .unwrap();
    assert!(matches!(
        PackageBuilder::build(request),
        Err(PipelineError::Package(_))
    ));
    assert!(!root.path().join("output").join(DATA_VERSION).exists());
}

fn decompress(path: &std::path::Path) -> Vec<u8> {
    zstd::stream::decode_all(fs::File::open(path).unwrap()).unwrap()
}

fn walk(root: &std::path::Path, current: &std::path::Path) -> Vec<String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(current).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk(root, &path));
        } else {
            files.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    files.sort();
    files
}
