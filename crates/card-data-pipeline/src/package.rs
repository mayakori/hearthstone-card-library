use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, Cursor, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use card_data_contract::{
    AssetDescriptor, CardCounts, DataVersion, LocaleManifest, Manifest, MINIMUM_APP_VERSION,
    SCHEMA_VERSION, SUPPORTED_LOCALES,
};
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};

use crate::PipelineError;

const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone)]
pub struct PackageLocaleInput {
    pub raw_bytes: Vec<u8>,
    pub sqlite_path: PathBuf,
    pub card_counts: CardCounts,
}

#[derive(Debug, Clone)]
pub struct PackageRequest {
    pub data_version: DataVersion,
    pub output_root: PathBuf,
    pub generated_at: String,
    pub locales: BTreeMap<String, PackageLocaleInput>,
}

#[derive(Debug, Clone)]
pub struct PackageResult {
    pub version_directory: PathBuf,
    pub manifest: Manifest,
}

pub struct PackageBuilder;

impl PackageBuilder {
    pub fn build(request: PackageRequest) -> Result<PackageResult, PipelineError> {
        Self::build_with_io(request, &FilesystemIo)
    }

    fn build_with_io(
        request: PackageRequest,
        io: &dyn PackageIo,
    ) -> Result<PackageResult, PipelineError> {
        validate_request(&request)?;
        let final_directory = request.output_root.join(request.data_version.to_string());
        if final_directory.exists() {
            return Err(PipelineError::Config(
                "package directory already exists".into(),
            ));
        }
        io.create_root(&request.output_root)?;
        let staging = io.create_staging(&request.output_root, &request.data_version)?;
        let result = build_in_staging(&request, &staging, io).and_then(|manifest| {
            validate_package_directory(&staging, &manifest)?;
            io.checkpoint(FaultPoint::FinalRename)?;
            if final_directory.exists() {
                return Err(PipelineError::Config(
                    "package directory already exists".into(),
                ));
            }
            if let Err(error) = io.rename(&staging, &final_directory) {
                if final_directory.exists() {
                    return Err(PipelineError::Config(
                        "package directory already exists".into(),
                    ));
                }
                return Err(error);
            }
            Ok(PackageResult {
                version_directory: final_directory,
                manifest,
            })
        });
        if result.is_err() && staging.exists() {
            let _ = io.remove_dir_all(&staging);
        }
        result
    }
}

fn build_in_staging(
    request: &PackageRequest,
    staging: &Path,
    io: &dyn PackageIo,
) -> Result<Manifest, PipelineError> {
    let raw_directory = staging.join("raw");
    let normalized_directory = staging.join("normalized");
    io.create_directory(&raw_directory)?;
    io.create_directory(&normalized_directory)?;
    let mut locales = BTreeMap::new();

    for locale in SUPPORTED_LOCALES {
        let input = request
            .locales
            .get(locale)
            .expect("validated supported locale input");
        let raw_hash = hash_bytes(&input.raw_bytes, io)?;
        verify_sqlite_metadata(
            &input.sqlite_path,
            &request.data_version,
            locale,
            &request.generated_at,
            &raw_hash,
            input.card_counts,
        )?;
        io.checkpoint(FaultPoint::RawWrite)?;
        let raw = compress_bytes(
            &input.raw_bytes,
            &raw_directory.join(format!("{locale}.json.zst")),
            io,
        )?;
        io.checkpoint(FaultPoint::SqliteWrite)?;
        let normalized = compress_file(
            &input.sqlite_path,
            &normalized_directory.join(format!("{locale}.sqlite.zst")),
            io,
        )?;
        locales.insert(
            locale.to_owned(),
            LocaleManifest {
                card_counts: input.card_counts,
                raw,
                normalized,
            },
        );
    }

    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        minimum_app_version: MINIMUM_APP_VERSION.into(),
        data_version: request.data_version.to_string(),
        official_patch_version: request.data_version.official_patch_version(),
        build_id: request.data_version.build_id(),
        revision: request.data_version.revision(),
        generated_at: request.generated_at.clone(),
        supported_locales: SUPPORTED_LOCALES.map(str::to_owned).to_vec(),
        locales,
    };
    manifest
        .validate()
        .map_err(|error| package_error(format!("invalid manifest: {error}")))?;
    io.checkpoint(FaultPoint::ManifestWrite)?;
    let mut bytes = serde_json::to_vec(&manifest)
        .map_err(|error| package_error(format!("serialize manifest: {error}")))?;
    bytes.push(b'\n');
    io.write_all(&staging.join(MANIFEST_FILE), &bytes)?;
    Ok(manifest)
}

pub fn validate_package_directory(
    directory: impl AsRef<Path>,
    expected_manifest: &Manifest,
) -> Result<(), PipelineError> {
    let directory = directory.as_ref();
    expected_manifest
        .validate()
        .map_err(|error| package_error(format!("invalid manifest: {error}")))?;
    require_exact_file_set(directory)?;
    let manifest_bytes = fs::read(directory.join(MANIFEST_FILE)).map_err(io_error)?;
    if !manifest_bytes.ends_with(b"\n")
        || manifest_bytes[..manifest_bytes.len() - 1].contains(&b'\n')
    {
        return Err(package_error(
            "manifest must be compact JSON with one trailing LF",
        ));
    }
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| package_error(format!("parse manifest: {error}")))?;
    if manifest != *expected_manifest {
        return Err(package_error("manifest does not match package result"));
    }
    manifest
        .validate()
        .map_err(|error| package_error(format!("invalid manifest: {error}")))?;
    let mut canonical_manifest_bytes = serde_json::to_vec(&manifest)
        .map_err(|error| package_error(format!("serialize manifest: {error}")))?;
    canonical_manifest_bytes.push(b'\n');
    if manifest_bytes != canonical_manifest_bytes {
        return Err(package_error("manifest is not canonical compact JSON"));
    }

    for locale in SUPPORTED_LOCALES {
        let assets = manifest
            .locales
            .get(locale)
            .expect("manifest validator requires supported locale");
        validate_asset(directory, locale, "raw", &assets.raw, false)?;
        validate_asset(directory, locale, "normalized", &assets.normalized, true)?;
        validate_compressed_sqlite_metadata(
            directory,
            &assets.normalized,
            &DataVersion::parse(&manifest.data_version)
                .map_err(|error| package_error(format!("invalid data version: {error}")))?,
            locale,
            &manifest.generated_at,
            &assets.raw.uncompressed_sha256,
            assets.card_counts,
        )?;
    }
    Ok(())
}

fn validate_compressed_sqlite_metadata(
    directory: &Path,
    asset: &AssetDescriptor,
    version: &DataVersion,
    locale: &str,
    generated_at: &str,
    source_raw_sha256: &str,
    counts: CardCounts,
) -> Result<(), PipelineError> {
    let temporary = unique_sibling(directory, ".validation")?;
    let result = (|| {
        let input = File::open(directory.join(&asset.path)).map_err(io_error)?;
        let mut decoder = zstd::stream::read::Decoder::new(input)
            .map_err(|error| package_error(format!("decompress SQLite: {error}")))?;
        let mut output = File::create(&temporary).map_err(io_error)?;
        io::copy(&mut decoder, &mut output).map_err(io_error)?;
        output.flush().map_err(io_error)?;
        verify_sqlite_metadata(
            &temporary,
            version,
            locale,
            generated_at,
            source_raw_sha256,
            counts,
        )
    })();
    let removal = fs::remove_file(&temporary);
    match (result, removal) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(io_error(error)),
    }
}

fn validate_asset(
    directory: &Path,
    locale: &str,
    kind: &str,
    asset: &AssetDescriptor,
    default_download: bool,
) -> Result<(), PipelineError> {
    let expected_path = match kind {
        "raw" => format!("raw/{locale}.json.zst"),
        "normalized" => format!("normalized/{locale}.sqlite.zst"),
        _ => return Err(package_error("unknown package asset kind")),
    };
    if asset.path != expected_path
        || asset.compression != "zstd"
        || asset.default_download != default_download
    {
        return Err(package_error(format!(
            "invalid {kind} asset descriptor for {locale}"
        )));
    }
    let path = directory.join(&asset.path);
    let compressed = hash_file(&path, &FilesystemIo)?;
    if compressed.bytes != asset.bytes || compressed.sha256 != asset.sha256 {
        return Err(package_error(format!(
            "compressed hash mismatch: {}",
            asset.path
        )));
    }
    let input = File::open(&path).map_err(io_error)?;
    let mut decoder = zstd::stream::read::Decoder::new(input)
        .map_err(|error| package_error(format!("decompress asset: {error}")))?;
    let uncompressed = hash_reader(&mut decoder, &FilesystemIo)?;
    if uncompressed.bytes != asset.uncompressed_bytes
        || uncompressed.sha256 != asset.uncompressed_sha256
    {
        return Err(package_error(format!(
            "uncompressed hash mismatch: {}",
            asset.path
        )));
    }
    Ok(())
}

fn require_exact_file_set(directory: &Path) -> Result<(), PipelineError> {
    if !directory.is_dir() {
        return Err(package_error("package directory is missing"));
    }
    let mut actual = BTreeSet::new();
    let mut directories = BTreeSet::new();
    collect_files(directory, directory, &mut actual, &mut directories)?;
    let expected_files = BTreeSet::from([
        MANIFEST_FILE.to_owned(),
        "raw/ko_KR.json.zst".to_owned(),
        "raw/en_US.json.zst".to_owned(),
        "normalized/ko_KR.sqlite.zst".to_owned(),
        "normalized/en_US.sqlite.zst".to_owned(),
    ]);
    let expected_directories = BTreeSet::from(["raw".to_owned(), "normalized".to_owned()]);
    if actual != expected_files || directories != expected_directories {
        return Err(package_error("package has an unexpected file set"));
    }
    Ok(())
}

fn collect_files(
    root: &Path,
    current: &Path,
    output: &mut BTreeSet<String>,
    directories: &mut BTreeSet<String>,
) -> Result<(), PipelineError> {
    for entry in fs::read_dir(current).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let path = entry.path();
        if path.is_dir() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| package_error("package path escaped root"))?;
            directories.insert(relative.to_string_lossy().replace('\\', "/"));
            collect_files(root, &path, output, directories)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| package_error("package path escaped root"))?;
            output.insert(relative.to_string_lossy().replace('\\', "/"));
        } else {
            return Err(package_error("package contains a non-file entry"));
        }
    }
    Ok(())
}

fn verify_sqlite_metadata(
    path: &Path,
    version: &DataVersion,
    locale: &str,
    generated_at: &str,
    source_raw_sha256: &str,
    counts: CardCounts,
) -> Result<(), PipelineError> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| package_error(format!("open SQLite metadata: {error}")))?;
    let values: (i64, String, String, String, String, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT schema_version, data_version, locale, generated_at, source_raw_sha256, standard_card_count, related_card_count, class_reference_card_count, total_card_count FROM catalog_metadata",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?)),
        )
        .map_err(|error| package_error(format!("read SQLite metadata: {error}")))?;
    let user_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| package_error(format!("read SQLite user_version: {error}")))?;
    if values.0 != i64::from(SCHEMA_VERSION)
        || user_version != i64::from(SCHEMA_VERSION)
        || values.1 != version.to_string()
        || values.2 != locale
        || values.3 != generated_at
        || values.4 != source_raw_sha256
        || (values.5, values.6, values.7, values.8)
            != (
                counts.standard as i64,
                counts.related as i64,
                counts.class_reference as i64,
                counts.total as i64,
            )
    {
        return Err(package_error(
            "SQLite metadata does not match package inputs",
        ));
    }
    Ok(())
}

fn compress_bytes(
    bytes: &[u8],
    destination: &Path,
    io: &dyn PackageIo,
) -> Result<AssetDescriptor, PipelineError> {
    compress_reader(Cursor::new(bytes), bytes.len() as u64, destination, io)
}

fn compress_file(
    source: &Path,
    destination: &Path,
    io: &dyn PackageIo,
) -> Result<AssetDescriptor, PipelineError> {
    let input = File::open(source).map_err(io_error)?;
    let bytes = input.metadata().map_err(io_error)?.len();
    compress_reader(input, bytes, destination, io)
}

fn compress_reader(
    mut source: impl Read,
    uncompressed_bytes: u64,
    destination: &Path,
    io: &dyn PackageIo,
) -> Result<AssetDescriptor, PipelineError> {
    io.checkpoint(FaultPoint::Compression)?;
    let output = io.create_file(destination)?;
    let mut encoder = zstd::stream::write::Encoder::new(output, 10)
        .map_err(|error| package_error(format!("initialize zstd encoder: {error}")))?;
    encoder
        .multithread(1)
        .map_err(|error| package_error(format!("configure zstd workers: {error}")))?;
    encoder.include_checksum(true).map_err(zstd_error)?;
    encoder.include_contentsize(true).map_err(zstd_error)?;
    encoder
        .set_pledged_src_size(Some(uncompressed_bytes))
        .map_err(zstd_error)?;
    io::copy(&mut source, &mut encoder).map_err(io_error)?;
    encoder.finish().map_err(zstd_error)?;
    let compressed = hash_file(destination, io)?;
    let uncompressed = match File::open(destination) {
        Ok(file) => {
            let mut decoder = zstd::stream::read::Decoder::new(file).map_err(|error| {
                package_error(format!("decompress newly written asset: {error}"))
            })?;
            hash_reader(&mut decoder, io)?
        }
        Err(error) => return Err(io_error(error)),
    };
    Ok(AssetDescriptor {
        path: destination
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| {
                if name.ends_with(".json.zst") {
                    format!("raw/{name}")
                } else {
                    format!("normalized/{name}")
                }
            })
            .ok_or_else(|| package_error("invalid asset destination"))?,
        bytes: compressed.bytes,
        sha256: compressed.sha256,
        compression: "zstd".into(),
        uncompressed_bytes: uncompressed.bytes,
        uncompressed_sha256: uncompressed.sha256,
        default_download: destination
            .parent()
            .and_then(|parent| parent.file_name())
            .is_some_and(|name| name == "normalized"),
    })
}

#[derive(Debug)]
struct Hash {
    bytes: u64,
    sha256: String,
}

fn hash_bytes(bytes: &[u8], io: &dyn PackageIo) -> Result<String, PipelineError> {
    Ok(hash_reader(&mut Cursor::new(bytes), io)?.sha256)
}

fn hash_file(path: &Path, io: &dyn PackageIo) -> Result<Hash, PipelineError> {
    let mut file = File::open(path).map_err(io_error)?;
    hash_reader(&mut file, io)
}

fn hash_reader(reader: &mut dyn Read, io: &dyn PackageIo) -> Result<Hash, PipelineError> {
    io.checkpoint(FaultPoint::Hash)?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 32 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(io_error)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        bytes += read as u64;
    }
    Ok(Hash {
        bytes,
        sha256: hex::encode(hasher.finalize()),
    })
}

fn validate_request(request: &PackageRequest) -> Result<(), PipelineError> {
    let generated_at = OffsetDateTime::parse(&request.generated_at, &Rfc3339)
        .map_err(|error| package_error(format!("generated_at must be RFC 3339: {error}")))?;
    if generated_at.offset() != UtcOffset::UTC {
        return Err(package_error("generated_at must use UTC"));
    }
    let locales = request.locales.keys().cloned().collect::<BTreeSet<_>>();
    let expected = SUPPORTED_LOCALES
        .map(str::to_owned)
        .into_iter()
        .collect::<BTreeSet<_>>();
    if locales != expected {
        return Err(package_error(
            "package inputs must include exactly ko_KR and en_US",
        ));
    }
    for (locale, input) in &request.locales {
        if !input.card_counts.is_valid() {
            return Err(package_error(format!("invalid card counts for {locale}")));
        }
        if !input.sqlite_path.is_file() {
            return Err(package_error(format!(
                "SQLite input is missing for {locale}"
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaultPoint {
    RawWrite,
    SqliteWrite,
    Compression,
    Hash,
    ManifestWrite,
    FinalRename,
}

trait PackageIo {
    fn checkpoint(&self, point: FaultPoint) -> Result<(), PipelineError>;
    fn create_root(&self, path: &Path) -> Result<(), PipelineError>;
    fn create_staging(&self, root: &Path, version: &DataVersion) -> Result<PathBuf, PipelineError>;
    fn create_directory(&self, path: &Path) -> Result<(), PipelineError>;
    fn create_file(&self, path: &Path) -> Result<File, PipelineError>;
    fn write_all(&self, path: &Path, bytes: &[u8]) -> Result<(), PipelineError>;
    fn rename(&self, from: &Path, to: &Path) -> Result<(), PipelineError>;
    fn remove_dir_all(&self, path: &Path) -> Result<(), PipelineError>;
}

struct FilesystemIo;

impl PackageIo for FilesystemIo {
    fn checkpoint(&self, _: FaultPoint) -> Result<(), PipelineError> {
        Ok(())
    }
    fn create_root(&self, path: &Path) -> Result<(), PipelineError> {
        fs::create_dir_all(path).map_err(io_error)
    }
    fn create_staging(&self, root: &Path, version: &DataVersion) -> Result<PathBuf, PipelineError> {
        for nonce in 0..100_u32 {
            let path = root.join(format!(".{}-staging-{}-{nonce}", version, unique_nonce()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(error)),
            }
        }
        Err(package_error(
            "could not create unique package staging directory",
        ))
    }
    fn create_directory(&self, path: &Path) -> Result<(), PipelineError> {
        fs::create_dir(path).map_err(io_error)
    }
    fn create_file(&self, path: &Path) -> Result<File, PipelineError> {
        File::create(path).map_err(io_error)
    }
    fn write_all(&self, path: &Path, bytes: &[u8]) -> Result<(), PipelineError> {
        fs::write(path, bytes).map_err(io_error)
    }
    fn rename(&self, from: &Path, to: &Path) -> Result<(), PipelineError> {
        no_replace_rename(from, to).map_err(io_error)
    }
    fn remove_dir_all(&self, path: &Path) -> Result<(), PipelineError> {
        fs::remove_dir_all(path).map_err(io_error)
    }
}

#[cfg(any(target_os = "linux", target_vendor = "apple"))]
fn no_replace_rename(from: &Path, to: &Path) -> io::Result<()> {
    use rustix::fs::{renameat_with, RenameFlags, CWD};

    Ok(renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE)?)
}

#[cfg(not(any(target_os = "linux", target_vendor = "apple")))]
fn no_replace_rename(from: &Path, to: &Path) -> io::Result<()> {
    // Windows std::fs::rename refuses an existing destination. Other targets retain
    // the pre-check and post-failure collision conversion in PackageBuilder.
    fs::rename(from, to)
}

fn unique_sibling(directory: &Path, suffix: &str) -> Result<PathBuf, PipelineError> {
    let parent = directory
        .parent()
        .ok_or_else(|| package_error("package directory has no parent"))?;
    for nonce in 0..100_u32 {
        let path = parent.join(format!(".package{suffix}-{}-{nonce}", unique_nonce()));
        if !path.exists() {
            return Ok(path);
        }
    }
    Err(package_error("could not allocate validation file"))
}

fn unique_nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
        ^ u128::from(std::process::id())
}

fn package_error(message: impl Into<String>) -> PipelineError {
    PipelineError::Package(message.into())
}
fn io_error(error: io::Error) -> PipelineError {
    PipelineError::Io(error.to_string())
}
fn zstd_error(error: io::Error) -> PipelineError {
    package_error(format!("zstd: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA_VERSION: &str = "36.0.3-build247416-r1";
    const GENERATED_AT: &str = "2026-08-05T00:00:00Z";

    #[test]
    fn injected_failures_remove_staging_and_never_publish_a_partial_package() {
        for point in [
            FaultPoint::RawWrite,
            FaultPoint::SqliteWrite,
            FaultPoint::Compression,
            FaultPoint::Hash,
            FaultPoint::ManifestWrite,
            FaultPoint::FinalRename,
        ] {
            let root = tempfile::tempdir().unwrap();
            let request = request(root.path());
            let final_directory = request.output_root.join(DATA_VERSION);
            assert!(PackageBuilder::build_with_io(request, &FailingIo { point }).is_err());
            assert!(!final_directory.exists(), "{point:?}");
            let residual = fs::read_dir(root.path().join("output")).unwrap().next();
            assert!(residual.is_none(), "{point:?}");
        }
    }

    #[test]
    fn rename_race_preserves_the_new_final_directory_and_reports_config_collision() {
        let root = tempfile::tempdir().unwrap();
        let request = request(root.path());
        let final_directory = request.output_root.join(DATA_VERSION);
        let result = PackageBuilder::build_with_io(request, &RenameRaceIo);

        assert!(matches!(result, Err(PipelineError::Config(_))));
        assert_eq!(
            fs::read(final_directory.join("race-owner.txt")).unwrap(),
            b"created by concurrent publisher\n"
        );
        let output_entries = fs::read_dir(root.path().join("output"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(output_entries, vec![std::ffi::OsString::from(DATA_VERSION)]);
    }

    fn request(root: &Path) -> PackageRequest {
        let input = root.join("input");
        fs::create_dir_all(&input).unwrap();
        let mut locales = BTreeMap::new();
        for locale in SUPPORTED_LOCALES {
            let raw_bytes = format!("{{\"locale\":\"{locale}\"}}\n").into_bytes();
            let sqlite_path = input.join(format!("{locale}.sqlite"));
            write_sqlite(&sqlite_path, locale, &raw_bytes);
            locales.insert(
                locale.into(),
                PackageLocaleInput {
                    raw_bytes,
                    sqlite_path,
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

    fn write_sqlite(path: &Path, locale: &str, raw: &[u8]) {
        let connection = Connection::open(path).unwrap();
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

    struct FailingIo {
        point: FaultPoint,
    }

    impl PackageIo for FailingIo {
        fn checkpoint(&self, point: FaultPoint) -> Result<(), PipelineError> {
            if point == self.point {
                Err(package_error(format!("injected failure at {point:?}")))
            } else {
                Ok(())
            }
        }
        fn create_root(&self, path: &Path) -> Result<(), PipelineError> {
            FilesystemIo.create_root(path)
        }
        fn create_staging(
            &self,
            root: &Path,
            version: &DataVersion,
        ) -> Result<PathBuf, PipelineError> {
            FilesystemIo.create_staging(root, version)
        }
        fn create_directory(&self, path: &Path) -> Result<(), PipelineError> {
            FilesystemIo.create_directory(path)
        }
        fn create_file(&self, path: &Path) -> Result<File, PipelineError> {
            FilesystemIo.create_file(path)
        }
        fn write_all(&self, path: &Path, bytes: &[u8]) -> Result<(), PipelineError> {
            FilesystemIo.write_all(path, bytes)
        }
        fn rename(&self, from: &Path, to: &Path) -> Result<(), PipelineError> {
            FilesystemIo.rename(from, to)
        }
        fn remove_dir_all(&self, path: &Path) -> Result<(), PipelineError> {
            FilesystemIo.remove_dir_all(path)
        }
    }

    struct RenameRaceIo;

    impl PackageIo for RenameRaceIo {
        fn checkpoint(&self, _: FaultPoint) -> Result<(), PipelineError> {
            Ok(())
        }
        fn create_root(&self, path: &Path) -> Result<(), PipelineError> {
            FilesystemIo.create_root(path)
        }
        fn create_staging(
            &self,
            root: &Path,
            version: &DataVersion,
        ) -> Result<PathBuf, PipelineError> {
            FilesystemIo.create_staging(root, version)
        }
        fn create_directory(&self, path: &Path) -> Result<(), PipelineError> {
            FilesystemIo.create_directory(path)
        }
        fn create_file(&self, path: &Path) -> Result<File, PipelineError> {
            FilesystemIo.create_file(path)
        }
        fn write_all(&self, path: &Path, bytes: &[u8]) -> Result<(), PipelineError> {
            FilesystemIo.write_all(path, bytes)
        }
        fn rename(&self, _: &Path, to: &Path) -> Result<(), PipelineError> {
            fs::create_dir(to).map_err(io_error)?;
            fs::write(
                to.join("race-owner.txt"),
                b"created by concurrent publisher\n",
            )
            .map_err(io_error)?;
            Err(io_error(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "injected final directory collision",
            )))
        }
        fn remove_dir_all(&self, path: &Path) -> Result<(), PipelineError> {
            FilesystemIo.remove_dir_all(path)
        }
    }
}
