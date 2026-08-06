use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Cursor, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use card_data_contract::Manifest;

use crate::{
    load_image_source, Event, EventSink, FetchedImage, ImageDownloadPolicy, ImageDownloader,
    ImageSource, ImageVariant, PipelineError,
};

const SCHEMA_VERSION: u32 = 1;
const PRODUCTION_PACK_LIMIT: usize = 480 * 1024 * 1024;

pub struct ImageCandidateRequest<'a> {
    pub output_root: &'a Path,
    pub source: &'a ImageSource,
    pub fetched: &'a [FetchedImage],
    pub run_id: &'a str,
    pub run_attempt: u32,
    pub input_manifest_sha256: &'a str,
    pub normalized_sha256: BTreeMap<String, String>,
    pub pack_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageCandidate {
    pub root: PathBuf,
    pub receipt: ImageReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageReceipt {
    pub schema_version: u32,
    pub data_version: String,
    pub run_id: String,
    pub run_attempt: u32,
    pub candidate_prefix: String,
    pub input_manifest_sha256: String,
    pub normalized_sha256: BTreeMap<String, String>,
    pub request_count: u64,
    pub absent_count: u64,
    pub unavailable_count: u64,
    pub success_count: u64,
    pub unique_image_count: u64,
    pub media_type_counts: BTreeMap<String, u64>,
    pub total_source_bytes: u64,
    pub packs: Vec<PackReceipt>,
    pub maps: Vec<ObjectReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectReceipt {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackReceipt {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub member_count: u64,
    pub unpacked_bytes: u64,
}

pub async fn run_image_baseline_build(
    package_root: impl AsRef<Path>,
    output_root: impl AsRef<Path>,
    run_id: &str,
    run_attempt: u32,
    events: &mut dyn EventSink,
) -> Result<ImageCandidate, PipelineError> {
    let package_root = package_root.as_ref();
    emit_event(events, Event::started("image_source"))?;
    let source = load_image_source(package_root)?;
    let manifest_bytes = fs::read(package_root.join("manifest.json")).map_err(io_error)?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| package_error(format!("parse image source manifest: {error}")))?;
    let normalized_sha256 = ["ko_KR", "en_US"]
        .into_iter()
        .map(|locale| {
            manifest
                .locales
                .get(locale)
                .map(|entry| (locale.into(), entry.normalized.sha256.clone()))
                .ok_or_else(|| package_error(format!("missing locale manifest: {locale}")))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    emit_event(events, Event::completed("image_source"))?;
    emit_event(events, Event::started("image_download"))?;
    let downloader = ImageDownloader::new(ImageDownloadPolicy::default())?;
    let fetched = downloader.download(&source.requests).await;
    for retry in downloader.take_retry_events() {
        emit_event(events, Event::image_retry(retry.attempt, retry.status_code))?;
    }
    let fetched = fetched?;
    emit_event(events, Event::completed("image_download"))?;
    emit_event(events, Event::started("image_package"))?;
    let candidate = build_image_candidate(ImageCandidateRequest {
        output_root: output_root.as_ref(),
        source: &source,
        fetched: &fetched,
        run_id,
        run_attempt,
        input_manifest_sha256: &sha256(&manifest_bytes),
        normalized_sha256,
        pack_limit: PRODUCTION_PACK_LIMIT,
    })?;
    let _ = Event::completed("image_package").and_then(|event| events.emit(event));
    let _ = Event::success().and_then(|event| events.emit(event));
    Ok(candidate)
}

fn emit_event(
    events: &mut dyn EventSink,
    event: Result<Event, std::io::Error>,
) -> Result<(), PipelineError> {
    events
        .emit(event.map_err(|error| PipelineError::Io(error.to_string()))?)
        .map_err(|error| PipelineError::Io(error.to_string()))
}

#[derive(Clone)]
struct CanonicalImage {
    bytes: Vec<u8>,
    media_type: String,
    extension: String,
    owner_locale: String,
    owner_variant: ImageVariant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocaleMap {
    schema_version: u32,
    data_version: String,
    locale: String,
    cards: Vec<CardMap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardMap {
    card_id: u64,
    normal: Option<AssetState>,
    crop: Option<AssetState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum AssetState {
    Available {
        #[serde(flatten)]
        asset: AssetMap,
    },
    Unavailable {
        source_url: String,
        reason: String,
        status_code: u16,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssetMap {
    source_url: String,
    sha256: String,
    bytes: u64,
    media_type: String,
    pack: String,
    member: String,
}

pub fn build_image_candidate(
    request: ImageCandidateRequest<'_>,
) -> Result<ImageCandidate, PipelineError> {
    validate_request(&request)?;
    let prefix = format!(
        "candidates/images/{}/runs/{}-{}",
        request.source.data_version, request.run_id, request.run_attempt
    );
    let final_root = request.output_root.join(Path::new(&prefix));
    if final_root.exists() {
        return Err(package_error("image candidate output already exists"));
    }
    fs::create_dir_all(request.output_root).map_err(io_error)?;
    let staging = tempfile::Builder::new()
        .prefix(".image-candidate-")
        .tempdir_in(request.output_root)
        .map_err(io_error)?;
    let root = staging.path().join(Path::new(&prefix));
    fs::create_dir_all(root.join("packs")).map_err(io_error)?;
    fs::create_dir_all(root.join("maps")).map_err(io_error)?;

    let canonical = canonical_images(request.fetched)?;
    let (packs, locations) = write_packs(&root, &canonical, request.pack_limit)?;
    let maps = write_maps(&root, request.source, request.fetched, &locations)?;

    let mut media_type_counts = BTreeMap::new();
    let mut total_source_bytes = 0_u64;
    for content in request
        .fetched
        .iter()
        .filter_map(|image| image.content.as_ref())
    {
        *media_type_counts
            .entry(content.media_type.clone())
            .or_insert(0) += 1;
        total_source_bytes += content.bytes.len() as u64;
    }
    let absent_count = request
        .fetched
        .iter()
        .filter(|image| image.content.is_none() && image.unavailable.is_none())
        .count() as u64;
    let unavailable_count = request
        .fetched
        .iter()
        .filter(|image| image.unavailable.is_some())
        .count() as u64;
    let receipt = ImageReceipt {
        schema_version: SCHEMA_VERSION,
        data_version: request.source.data_version.clone(),
        run_id: request.run_id.into(),
        run_attempt: request.run_attempt,
        candidate_prefix: prefix,
        input_manifest_sha256: request.input_manifest_sha256.into(),
        normalized_sha256: request.normalized_sha256,
        request_count: request.fetched.len() as u64,
        absent_count,
        unavailable_count,
        success_count: request.fetched.len() as u64 - absent_count - unavailable_count,
        unique_image_count: canonical.len() as u64,
        media_type_counts,
        total_source_bytes,
        packs,
        maps,
    };
    let bytes = serde_json::to_vec(&receipt)
        .map_err(|error| package_error(format!("serialize image receipt: {error}")))?;
    fs::write(root.join("receipt.json"), bytes).map_err(io_error)?;
    verify_image_candidate(&root)?;
    fs::create_dir_all(
        final_root
            .parent()
            .ok_or_else(|| package_error("image candidate has no parent directory"))?,
    )
    .map_err(io_error)?;
    fs::rename(&root, &final_root).map_err(io_error)?;
    Ok(ImageCandidate {
        root: final_root,
        receipt,
    })
}

fn validate_request(request: &ImageCandidateRequest<'_>) -> Result<(), PipelineError> {
    if request.pack_limit == 0
        || !is_positive_decimal(request.run_id)
        || request.run_attempt == 0
        || !is_sha256(request.input_manifest_sha256)
        || request.normalized_sha256.len() != 2
        || !["ko_KR", "en_US"].into_iter().all(|locale| {
            request
                .normalized_sha256
                .get(locale)
                .is_some_and(|hash| is_sha256(hash))
        })
    {
        return Err(package_error(
            "invalid image candidate build identity, digest or limit",
        ));
    }
    if request.source.requests.len() != request.fetched.len() {
        return Err(package_error("image source/fetch count mismatch"));
    }
    for (source, fetched) in request.source.requests.iter().zip(request.fetched) {
        let outcome_count =
            usize::from(fetched.content.is_some()) + usize::from(fetched.unavailable.is_some());
        let outcome_matches_source = match source.source_url {
            Some(_) => outcome_count == 1,
            None => outcome_count == 0,
        };
        if source != &fetched.request || !outcome_matches_source {
            return Err(package_error("image source/fetch alignment mismatch"));
        }
    }
    Ok(())
}

fn canonical_images(
    fetched: &[FetchedImage],
) -> Result<BTreeMap<String, CanonicalImage>, PipelineError> {
    let mut canonical = BTreeMap::new();
    for image in fetched {
        let Some(content) = &image.content else {
            continue;
        };
        let actual_hash = sha256(&content.bytes);
        if actual_hash != content.sha256 {
            return Err(package_error("downloaded image SHA-256 mismatch"));
        }
        let expected_extension = match content.media_type.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/webp" => "webp",
            _ => return Err(package_error("unsupported downloaded image media type")),
        };
        if content.extension != expected_extension {
            return Err(package_error("downloaded image extension mismatch"));
        }
        match canonical.get(&actual_hash) {
            Some(existing) => {
                let existing: &CanonicalImage = existing;
                if existing.media_type != content.media_type
                    || existing.extension != content.extension
                    || existing.bytes != content.bytes
                {
                    return Err(package_error("conflicting canonical image hash"));
                }
            }
            None => {
                canonical.insert(
                    actual_hash,
                    CanonicalImage {
                        bytes: content.bytes.clone(),
                        media_type: content.media_type.clone(),
                        extension: content.extension.clone(),
                        owner_locale: image.request.locale.clone(),
                        owner_variant: image.request.variant,
                    },
                );
            }
        }
    }
    Ok(canonical)
}

fn write_packs(
    root: &Path,
    canonical: &BTreeMap<String, CanonicalImage>,
    pack_limit: usize,
) -> Result<(Vec<PackReceipt>, BTreeMap<String, (String, String)>), PipelineError> {
    let mut receipts = Vec::new();
    let mut locations = BTreeMap::new();
    for locale in ["ko_KR", "en_US"] {
        for variant in [ImageVariant::Normal, ImageVariant::Crop] {
            let hashes = canonical
                .iter()
                .filter_map(|(hash, image)| {
                    (image.owner_locale == locale && image.owner_variant == variant)
                        .then_some(hash.clone())
                })
                .collect::<Vec<_>>();
            let shards = shard_hashes(&hashes, canonical, pack_limit)?;
            for (shard, hashes) in shards.into_iter().enumerate() {
                let name = format!("{}-{}-{shard:03}.tar.zst", locale, variant_name(variant));
                let relative_path = format!("packs/{name}");
                let bytes = pack_bytes(&hashes, canonical)?;
                if bytes.len() > pack_limit {
                    return Err(package_error("image pack exceeds shard limit"));
                }
                fs::write(root.join(Path::new(&relative_path)), &bytes).map_err(io_error)?;
                let unpacked_bytes = hashes
                    .iter()
                    .map(|hash| canonical[hash].bytes.len() as u64)
                    .sum();
                for hash in &hashes {
                    locations.insert(
                        hash.clone(),
                        (
                            relative_path.clone(),
                            format!("{}.{}", hash, canonical[hash].extension),
                        ),
                    );
                }
                receipts.push(PackReceipt {
                    path: relative_path,
                    bytes: bytes.len() as u64,
                    sha256: sha256(&bytes),
                    member_count: hashes.len() as u64,
                    unpacked_bytes,
                });
            }
        }
    }
    Ok((receipts, locations))
}

fn shard_hashes(
    hashes: &[String],
    canonical: &BTreeMap<String, CanonicalImage>,
    pack_limit: usize,
) -> Result<Vec<Vec<String>>, PipelineError> {
    if hashes.is_empty() {
        return Ok(Vec::new());
    }
    let mut provisional = Vec::new();
    let mut current = Vec::new();
    let mut estimated = 1024_usize;
    for hash in hashes {
        let member_size = 512 + canonical[hash].bytes.len().div_ceil(512) * 512;
        if !current.is_empty() && estimated.saturating_add(member_size) > pack_limit {
            provisional.push(std::mem::take(&mut current));
            estimated = 1024;
        }
        current.push(hash.clone());
        estimated = estimated.saturating_add(member_size);
    }
    if !current.is_empty() {
        provisional.push(current);
    }
    let mut result = Vec::new();
    for shard in provisional {
        split_oversized(shard, canonical, pack_limit, &mut result)?;
    }
    Ok(result)
}

fn split_oversized(
    hashes: Vec<String>,
    canonical: &BTreeMap<String, CanonicalImage>,
    pack_limit: usize,
    output: &mut Vec<Vec<String>>,
) -> Result<(), PipelineError> {
    if pack_bytes(&hashes, canonical)?.len() <= pack_limit {
        output.push(hashes);
        return Ok(());
    }
    if hashes.len() == 1 {
        return Err(package_error("single image exceeds pack shard limit"));
    }
    let midpoint = hashes.len() / 2;
    split_oversized(hashes[..midpoint].to_vec(), canonical, pack_limit, output)?;
    split_oversized(hashes[midpoint..].to_vec(), canonical, pack_limit, output)
}

fn pack_bytes(
    hashes: &[String],
    canonical: &BTreeMap<String, CanonicalImage>,
) -> Result<Vec<u8>, PipelineError> {
    let mut tar_bytes = Vec::new();
    for hash in hashes {
        let image = &canonical[hash];
        let name = format!("{}.{}", hash, image.extension);
        append_tar_file(&mut tar_bytes, &name, &image.bytes)?;
    }
    tar_bytes.extend_from_slice(&[0_u8; 1024]);
    compress(&tar_bytes)
}

fn append_tar_file(output: &mut Vec<u8>, name: &str, bytes: &[u8]) -> Result<(), PipelineError> {
    let header = tar_header(name, bytes.len() as u64)?;
    output.extend_from_slice(&header);
    output.extend_from_slice(bytes);
    output.resize(output.len().div_ceil(512) * 512, 0);
    Ok(())
}

fn tar_header(name: &str, size: u64) -> Result<[u8; 512], PipelineError> {
    if name.len() > 100 || name.contains('/') || !name.is_ascii() {
        return Err(package_error("invalid image tar member name"));
    }
    let mut header = [0_u8; 512];
    header[..name.len()].copy_from_slice(name.as_bytes());
    write_tar_octal(&mut header[100..108], 0o644)?;
    write_tar_octal(&mut header[108..116], 0)?;
    write_tar_octal(&mut header[116..124], 0)?;
    write_tar_octal(&mut header[124..136], size)?;
    write_tar_octal(&mut header[136..148], 0)?;
    header[148..156].fill(b' ');
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let checksum: u64 = header.iter().map(|byte| *byte as u64).sum();
    let checksum_text = format!("{checksum:06o}\0 ");
    header[148..156].copy_from_slice(checksum_text.as_bytes());
    Ok(header)
}

fn write_tar_octal(field: &mut [u8], value: u64) -> Result<(), PipelineError> {
    let text = format!("{:0width$o}\0", value, width = field.len() - 1);
    if text.len() != field.len() {
        return Err(package_error("image tar numeric field overflow"));
    }
    field.copy_from_slice(text.as_bytes());
    Ok(())
}

fn write_maps(
    root: &Path,
    source: &ImageSource,
    fetched: &[FetchedImage],
    locations: &BTreeMap<String, (String, String)>,
) -> Result<Vec<ObjectReceipt>, PipelineError> {
    let mut receipts = Vec::new();
    for locale in ["ko_KR", "en_US"] {
        let mut by_card: BTreeMap<u64, CardMap> = BTreeMap::new();
        for image in fetched
            .iter()
            .filter(|image| image.request.locale == locale)
        {
            let entry = by_card.entry(image.request.card_id).or_insert(CardMap {
                card_id: image.request.card_id,
                normal: None,
                crop: None,
            });
            let asset =
                match (&image.content, &image.unavailable) {
                    (Some(content), None) => {
                        let (pack, member) = locations.get(&content.sha256).ok_or_else(|| {
                            package_error("image map has no canonical pack location")
                        })?;
                        Some(AssetState::Available {
                            asset: AssetMap {
                                source_url: image.request.source_url.clone().ok_or_else(|| {
                                    package_error("downloaded image is missing source URL")
                                })?,
                                sha256: content.sha256.clone(),
                                bytes: content.bytes.len() as u64,
                                media_type: content.media_type.clone(),
                                pack: pack.clone(),
                                member: member.clone(),
                            },
                        })
                    }
                    (None, Some(unavailable)) => Some(AssetState::Unavailable {
                        source_url: image.request.source_url.clone().ok_or_else(|| {
                            package_error("unavailable image is missing source URL")
                        })?,
                        reason: unavailable.reason.clone(),
                        status_code: unavailable.status_code,
                    }),
                    (None, None) => None,
                    (Some(_), Some(_)) => {
                        return Err(package_error("image has conflicting fetch outcomes"));
                    }
                };
            match image.request.variant {
                ImageVariant::Normal => entry.normal = asset,
                ImageVariant::Crop => entry.crop = asset,
            }
        }
        let locale_map = LocaleMap {
            schema_version: SCHEMA_VERSION,
            data_version: source.data_version.clone(),
            locale: locale.into(),
            cards: by_card.into_values().collect(),
        };
        let json = serde_json::to_vec(&locale_map)
            .map_err(|error| package_error(format!("serialize image map: {error}")))?;
        let bytes = compress(&json)?;
        let path = format!("maps/{locale}.json.zst");
        fs::write(root.join(Path::new(&path)), &bytes).map_err(io_error)?;
        receipts.push(ObjectReceipt {
            path,
            bytes: bytes.len() as u64,
            sha256: sha256(&bytes),
        });
    }
    Ok(receipts)
}

pub fn verify_image_candidate(root: impl AsRef<Path>) -> Result<ImageReceipt, PipelineError> {
    let root = root.as_ref();
    let receipt_bytes = fs::read(root.join("receipt.json")).map_err(io_error)?;
    let receipt: ImageReceipt = serde_json::from_slice(&receipt_bytes)
        .map_err(|error| package_error(format!("parse image receipt: {error}")))?;
    let canonical = serde_json::to_vec(&receipt)
        .map_err(|error| package_error(format!("serialize image receipt: {error}")))?;
    if canonical != receipt_bytes {
        return Err(package_error("image receipt is not canonical JSON"));
    }
    let expected_prefix = format!(
        "candidates/images/{}/runs/{}-{}",
        receipt.data_version, receipt.run_id, receipt.run_attempt
    );
    if receipt.schema_version != SCHEMA_VERSION
        || receipt.candidate_prefix != expected_prefix
        || receipt.run_id.is_empty()
        || receipt.run_attempt == 0
        || !is_sha256(&receipt.input_manifest_sha256)
        || receipt.normalized_sha256.len() != 2
        || !["ko_KR", "en_US"].into_iter().all(|locale| {
            receipt
                .normalized_sha256
                .get(locale)
                .is_some_and(|hash| is_sha256(hash))
        })
    {
        return Err(package_error("image receipt identity is invalid"));
    }

    let mut members: BTreeMap<(String, String), (String, u64)> = BTreeMap::new();
    let mut member_hashes = BTreeSet::new();
    let mut pack_paths = BTreeSet::new();
    for pack in &receipt.packs {
        if !pack_paths.insert(pack.path.clone()) {
            return Err(package_error("duplicate image pack path"));
        }
        let bytes = verified_object(root, &pack.path, pack.bytes, &pack.sha256)?;
        let pack_members = verify_pack_bytes(&bytes)?;
        if pack_members.len() as u64 != pack.member_count
            || pack_members.values().map(|(_, bytes)| *bytes).sum::<u64>() != pack.unpacked_bytes
        {
            return Err(package_error("image pack aggregate mismatch"));
        }
        for (member, (hash, size)) in pack_members {
            if !member_hashes.insert(hash.clone()) {
                return Err(package_error("canonical image appears in multiple packs"));
            }
            if members
                .insert((pack.path.clone(), member), (hash, size))
                .is_some()
            {
                return Err(package_error("duplicate image pack member"));
            }
        }
    }
    let mut request_count = 0_u64;
    let mut absent_count = 0_u64;
    let mut unavailable_count = 0_u64;
    let mut success_count = 0_u64;
    let mut media_type_counts = BTreeMap::<String, u64>::new();
    let mut total_source_bytes = 0_u64;
    let mut referenced_hashes = BTreeSet::new();
    let mut hash_metadata: BTreeMap<String, (String, u64)> = BTreeMap::new();
    let mut map_locales = BTreeSet::new();
    for map in &receipt.maps {
        let bytes = verified_object(root, &map.path, map.bytes, &map.sha256)?;
        let json = zstd::stream::decode_all(Cursor::new(bytes))
            .map_err(|error| package_error(format!("decompress image map: {error}")))?;
        let locale_map: LocaleMap = serde_json::from_slice(&json)
            .map_err(|error| package_error(format!("parse image map: {error}")))?;
        let canonical_json = serde_json::to_vec(&locale_map)
            .map_err(|error| package_error(format!("serialize image map: {error}")))?;
        if canonical_json != json || locale_map.data_version != receipt.data_version {
            return Err(package_error(
                "image map identity or canonical JSON mismatch",
            ));
        }
        if map.path != format!("maps/{}.json.zst", locale_map.locale)
            || !["ko_KR", "en_US"].contains(&locale_map.locale.as_str())
            || !map_locales.insert(locale_map.locale.clone())
        {
            return Err(package_error("image map locale/path mismatch"));
        }
        let mut card_ids = BTreeSet::new();
        let mut previous_card_id = None;
        for card in locale_map.cards {
            if !card_ids.insert(card.card_id)
                || previous_card_id.is_some_and(|previous| previous >= card.card_id)
            {
                return Err(package_error("duplicate image map card ID"));
            }
            previous_card_id = Some(card.card_id);
            for asset in [card.normal, card.crop] {
                request_count += 1;
                match asset {
                    Some(AssetState::Available { asset }) => {
                        success_count += 1;
                        *media_type_counts
                            .entry(asset.media_type.clone())
                            .or_insert(0) += 1;
                        total_source_bytes += asset.bytes;
                        let (hash, member_bytes) = members
                            .get(&(asset.pack, asset.member.clone()))
                            .ok_or_else(|| package_error("dangling image map pack reference"))?;
                        if hash != &asset.sha256
                            || !is_sha256(&asset.sha256)
                            || *member_bytes != asset.bytes
                        {
                            return Err(package_error("image map member SHA-256 mismatch"));
                        }
                        let expected_extension = match asset.media_type.as_str() {
                            "image/png" => "png",
                            "image/jpeg" => "jpg",
                            "image/webp" => "webp",
                            _ => return Err(package_error("image map media type is invalid")),
                        };
                        if !asset.member.ends_with(&format!(".{expected_extension}")) {
                            return Err(package_error("image map member extension mismatch"));
                        }
                        referenced_hashes.insert(asset.sha256.clone());
                        match hash_metadata.get(&asset.sha256) {
                            Some((media_type, bytes))
                                if media_type != &asset.media_type || *bytes != asset.bytes =>
                            {
                                return Err(package_error("image map hash metadata conflict"));
                            }
                            None => {
                                hash_metadata
                                    .insert(asset.sha256.clone(), (asset.media_type, asset.bytes));
                            }
                            _ => {}
                        }
                    }
                    Some(AssetState::Unavailable {
                        source_url,
                        reason,
                        status_code,
                    }) => {
                        if source_url.is_empty()
                            || reason != "http_status"
                            || !(400..500).contains(&status_code)
                            || status_code == 429
                        {
                            return Err(package_error("image unavailable state is invalid"));
                        }
                        unavailable_count += 1;
                    }
                    None => absent_count += 1,
                }
            }
        }
    }
    if map_locales != BTreeSet::from(["en_US".to_owned(), "ko_KR".to_owned()])
        || request_count != receipt.request_count
        || absent_count != receipt.absent_count
        || unavailable_count != receipt.unavailable_count
        || success_count != receipt.success_count
        || referenced_hashes != member_hashes
        || referenced_hashes.len() as u64 != receipt.unique_image_count
        || total_source_bytes != receipt.total_source_bytes
        || media_type_counts != receipt.media_type_counts
    {
        return Err(package_error("image receipt request aggregate mismatch"));
    }
    Ok(receipt)
}

fn verified_object(
    root: &Path,
    relative: &str,
    expected_bytes: u64,
    expected_hash: &str,
) -> Result<Vec<u8>, PipelineError> {
    let path = safe_relative(relative)?;
    let bytes = fs::read(root.join(path)).map_err(io_error)?;
    if bytes.len() as u64 != expected_bytes || sha256(&bytes) != expected_hash {
        return Err(package_error("image candidate object digest mismatch"));
    }
    Ok(bytes)
}

fn verify_pack_bytes(bytes: &[u8]) -> Result<BTreeMap<String, (String, u64)>, PipelineError> {
    let tar_bytes = zstd::stream::decode_all(Cursor::new(bytes))
        .map_err(|error| package_error(format!("decompress image pack: {error}")))?;
    let mut members = BTreeMap::new();
    let mut offset = 0_usize;
    let mut found_end = false;
    while offset + 512 <= tar_bytes.len() {
        let header = &tar_bytes[offset..offset + 512];
        if header.iter().all(|byte| *byte == 0) {
            found_end = tar_bytes[offset..].iter().all(|byte| *byte == 0)
                && tar_bytes.len() - offset >= 1024;
            break;
        }
        let name_end = header[..100]
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(100);
        let name = std::str::from_utf8(&header[..name_end])
            .map_err(|_| package_error("image tar member name is not UTF-8"))?
            .to_owned();
        let path = Path::new(&name);
        if path.components().count() != 1
            || !matches!(path.components().next(), Some(Component::Normal(_)))
        {
            return Err(package_error("unsafe image pack member path"));
        }
        let size = parse_tar_octal(&header[124..136])? as usize;
        if header != tar_header(&name, size as u64)? {
            return Err(package_error("image tar header is not deterministic"));
        }
        let content_start = offset + 512;
        let content_end = content_start
            .checked_add(size)
            .ok_or_else(|| package_error("image tar member size overflow"))?;
        if content_end > tar_bytes.len() {
            return Err(package_error("truncated image tar member"));
        }
        let content = &tar_bytes[content_start..content_end];
        let hash = sha256(&content);
        if !name.starts_with(&hash) {
            return Err(package_error("image pack member name/hash mismatch"));
        }
        if members.insert(name, (hash, size as u64)).is_some() {
            return Err(package_error("duplicate image pack member"));
        }
        let next_offset = content_end.div_ceil(512) * 512;
        if tar_bytes[content_end..next_offset]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(package_error("image tar member padding is not zeroed"));
        }
        offset = next_offset;
    }
    if !found_end {
        return Err(package_error("image tar end marker is missing"));
    }
    Ok(members)
}

fn parse_tar_octal(field: &[u8]) -> Result<u64, PipelineError> {
    let text = std::str::from_utf8(field)
        .map_err(|_| package_error("image tar numeric field is invalid"))?
        .trim_matches(['\0', ' ']);
    u64::from_str_radix(text, 8).map_err(|_| package_error("image tar numeric field is invalid"))
}

fn safe_relative(value: &str) -> Result<PathBuf, PipelineError> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(package_error("unsafe image candidate object path"));
    }
    Ok(path.to_owned())
}

fn compress(bytes: &[u8]) -> Result<Vec<u8>, PipelineError> {
    let mut encoder = zstd::stream::Encoder::new(Vec::new(), 3)
        .map_err(|error| package_error(format!("create zstd encoder: {error}")))?;
    encoder
        .include_checksum(true)
        .map_err(|error| package_error(format!("configure zstd checksum: {error}")))?;
    encoder
        .set_pledged_src_size(Some(bytes.len() as u64))
        .map_err(|error| package_error(format!("configure zstd content size: {error}")))?;
    encoder.write_all(bytes).map_err(io_error)?;
    encoder
        .finish()
        .map_err(|error| package_error(format!("finish zstd stream: {error}")))
}

fn variant_name(variant: ImageVariant) -> &'static str {
    match variant {
        ImageVariant::Normal => "normal",
        ImageVariant::Crop => "crop",
    }
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_positive_decimal(value: &str) -> bool {
    value.as_bytes().split_first().is_some_and(|(first, rest)| {
        (b'1'..=b'9').contains(first) && rest.iter().all(u8::is_ascii_digit)
    })
}

fn package_error(message: impl Into<String>) -> PipelineError {
    PipelineError::Package(message.into())
}

fn io_error(error: std::io::Error) -> PipelineError {
    PipelineError::Io(error.to_string())
}
