from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import time
import urllib.request
import zipfile
from collections import defaultdict
from pathlib import Path, PurePosixPath
from typing import NamedTuple


LOCALES = ("ko_KR", "en_US")
SOURCE_PREFIX_PATTERN = re.compile(
    r"^candidates/images/(\d+\.\d+\.\d+-build\d+-r[1-9]\d*)/runs/([1-9]\d*)-([1-9]\d*)$"
)
DERIVED_PREFIX_PATTERN = re.compile(
    r"^candidates/derived-images/realesrgan-x2/(\d+\.\d+\.\d+-build\d+-r[1-9]\d*)/runs/([1-9]\d*)-([1-9]\d*)$"
)
PACK_PATTERN = re.compile(r"^packs/(ko_KR|en_US)-(normal|crop)-\d{3}\.tar\.zst$")
MAP_PATTERN = re.compile(r"^maps/(ko_KR|en_US)\.json\.zst$")
SOURCE_MEMBER_PATTERN = re.compile(r"^([0-9a-f]{64})\.(png|jpg|webp)$")
OUTPUT_MEMBER_PATTERN = re.compile(r"^([0-9a-f]{64})\.png$")
HASH_PATTERN = re.compile(r"^[0-9a-f]{64}$")

TOOL_URL = (
    "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.5.0/"
    "realesrgan-ncnn-vulkan-20220424-windows.zip"
)
TOOL_ARCHIVE_SHA256 = "abc02804e17982a3be33675e4d471e91ea374e65b70167abc09e31acb412802d"
TOOL_EXE_SHA256 = "07e49f7cbb4ede01ae4dd4c399d3a7e5846e3d2085c3128eff881e55cb7b1a0c"
MODEL_PARAM_SHA256 = "35330ececcea33b6c397a72548e788d5d53becee4734c50b7fada36e89f10a86"
MODEL_BIN_SHA256 = "713ee713b0353afaa27976f0563a64a5043bd70b9bd8936c2e26e25ebcdbcddf"


class SourceIdentity(NamedTuple):
    data_version: str
    run_id: str
    run_attempt: int


def emit(event: str, **fields: object) -> None:
    record = {"event": event, **fields}
    print(json.dumps(record, ensure_ascii=False, separators=(",", ":")), flush=True)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path, chunk_size: int = 4 * 1024 * 1024) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(chunk_size):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")
        + b"\n"
    )


def parse_source_prefix(prefix: str) -> SourceIdentity:
    match = SOURCE_PREFIX_PATTERN.fullmatch(prefix)
    if match is None:
        raise ValueError("source candidate prefix is invalid")
    return SourceIdentity(match.group(1), match.group(2), int(match.group(3)))


def _checked_object(entry: object, pattern: re.Pattern[str], kind: str) -> dict:
    if not isinstance(entry, dict):
        raise ValueError(f"source {kind} receipt entry is invalid")
    path = entry.get("path")
    byte_count = entry.get("bytes")
    digest = entry.get("sha256")
    if (
        not isinstance(path, str)
        or pattern.fullmatch(path) is None
        or not isinstance(byte_count, int)
        or isinstance(byte_count, bool)
        or byte_count <= 0
        or not isinstance(digest, str)
        or HASH_PATTERN.fullmatch(digest) is None
    ):
        raise ValueError(f"source {kind} receipt entry is invalid")
    return entry


def validate_source_receipt(receipt: object, source_prefix: str) -> tuple[list[dict], list[dict]]:
    identity = parse_source_prefix(source_prefix)
    if not isinstance(receipt, dict):
        raise ValueError("source receipt is invalid")
    if (
        receipt.get("schemaVersion") != 1
        or receipt.get("candidatePrefix") != source_prefix
        or receipt.get("dataVersion") != identity.data_version
        or str(receipt.get("runId")) != identity.run_id
        or receipt.get("runAttempt") != identity.run_attempt
    ):
        raise ValueError("source receipt identity is invalid")
    packs_raw = receipt.get("packs")
    maps_raw = receipt.get("maps")
    if not isinstance(packs_raw, list) or not isinstance(maps_raw, list):
        raise ValueError("source receipt object lists are invalid")
    packs = [_checked_object(entry, PACK_PATTERN, "pack") for entry in packs_raw]
    maps = [_checked_object(entry, MAP_PATTERN, "map") for entry in maps_raw]
    paths = [entry["path"] for entry in [*packs, *maps]]
    if len(paths) != len(set(paths)):
        raise ValueError("source receipt contains duplicate object paths")
    if {entry["path"] for entry in maps} != {"maps/ko_KR.json.zst", "maps/en_US.json.zst"}:
        raise ValueError("source receipt map set is invalid")
    for pack in packs:
        if (
            not isinstance(pack.get("memberCount"), int)
            or pack["memberCount"] <= 0
            or not isinstance(pack.get("unpackedBytes"), int)
            or pack["unpackedBytes"] <= 0
        ):
            raise ValueError("source pack aggregate is invalid")
    return packs, maps


def required_pack_entries(packs: list[dict], references: list[dict]) -> list[dict]:
    by_path = {entry["path"]: entry for entry in packs}
    required_paths = sorted({reference["sourcePack"] for reference in references})
    missing = [path for path in required_paths if path not in by_path]
    if missing:
        raise ValueError("normal map references a pack absent from source receipt")
    return [by_path[path] for path in required_paths]


def select_assets(assets: list[dict], max_images: int) -> list[dict]:
    if max_images < 0:
        raise ValueError("max_images must be zero or positive")
    selected: list[dict] = []
    for locale in LOCALES:
        locale_assets = sorted(
            (asset for asset in assets if asset.get("ownerLocale") == locale),
            key=lambda asset: asset["sourceSha256"],
        )
        selected.extend(locale_assets if max_images == 0 else locale_assets[:max_images])
    return selected


def select_reference_hashes(references: list[dict], max_images: int) -> set[str]:
    if max_images < 0:
        raise ValueError("max_images must be zero or positive")
    selected: set[str] = set()
    for locale in LOCALES:
        hashes = sorted({item["sourceSha256"] for item in references if item["locale"] == locale})
        selected.update(hashes if max_images == 0 else hashes[:max_images])
    return selected


def validate_source_member(member: str, payload: bytes) -> tuple[str, str]:
    if PurePosixPath(member).name != member:
        raise ValueError("source pack member path is unsafe")
    match = SOURCE_MEMBER_PATTERN.fullmatch(member)
    if match is None or sha256_bytes(payload) != match.group(1):
        raise ValueError("source pack member SHA-256 is invalid")
    return match.group(1), match.group(2)


def _validate_reference(reference: dict) -> None:
    if (
        reference.get("locale") not in LOCALES
        or not isinstance(reference.get("cardId"), int)
        or reference["cardId"] <= 0
        or HASH_PATTERN.fullmatch(reference.get("sourceSha256", "")) is None
        or PACK_PATTERN.fullmatch(reference.get("sourcePack", "")) is None
        or SOURCE_MEMBER_PATTERN.fullmatch(reference.get("sourceMember", "")) is None
        or not isinstance(reference.get("sourceBytes"), int)
        or reference["sourceBytes"] <= 0
    ):
        raise ValueError("source normal reference is invalid")
    if not reference["sourceMember"].startswith(reference["sourceSha256"] + "."):
        raise ValueError("source normal reference member/hash mismatch")


def _zstd_module():
    try:
        import zstandard
    except ImportError as error:
        raise RuntimeError("zstandard dependency is required") from error
    return zstandard


def _pillow_image():
    try:
        from PIL import Image
    except ImportError as error:
        raise RuntimeError("Pillow dependency is required") from error
    return Image


def read_zstd_json(path: Path) -> dict:
    zstandard = _zstd_module()
    decoded = zstandard.ZstdDecompressor().decompress(path.read_bytes())
    value = json.loads(decoded)
    if not isinstance(value, dict):
        raise ValueError("zstd JSON root is invalid")
    return value


def source_normal_references(map_path: Path, expected_locale: str, data_version: str) -> list[dict]:
    value = read_zstd_json(map_path)
    if (
        value.get("schemaVersion") != 1
        or value.get("dataVersion") != data_version
        or value.get("locale") != expected_locale
        or not isinstance(value.get("cards"), list)
    ):
        raise ValueError("source locale map identity is invalid")
    references: list[dict] = []
    previous_card_id = 0
    for card in value["cards"]:
        if not isinstance(card, dict) or not isinstance(card.get("cardId"), int):
            raise ValueError("source locale map card is invalid")
        card_id = card["cardId"]
        if card_id <= previous_card_id:
            raise ValueError("source locale map cards are not strictly ordered")
        previous_card_id = card_id
        normal = card.get("normal")
        if normal is None or normal.get("state") == "unavailable":
            continue
        if not isinstance(normal, dict) or normal.get("state") != "available":
            raise ValueError("source normal state is invalid")
        reference = {
            "locale": expected_locale,
            "cardId": card_id,
            "sourceSha256": normal.get("sha256"),
            "sourcePack": normal.get("pack"),
            "sourceMember": normal.get("member"),
            "sourceBytes": normal.get("bytes"),
        }
        _validate_reference(reference)
        references.append(reference)
    return references


def verify_file_entry(root: Path, entry: dict) -> Path:
    path = root.joinpath(*entry["path"].split("/"))
    if not path.is_file() or path.stat().st_size != entry["bytes"] or sha256_file(path) != entry["sha256"]:
        raise ValueError(f"object verification failed: {entry['path']}")
    return path


def finalize_x2(source_path: Path, enhanced_path: Path, output_path: Path) -> None:
    Image = _pillow_image()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    temporary = output_path.with_suffix(".tmp.png")
    with Image.open(source_path) as source_file, Image.open(enhanced_path) as enhanced_file:
        source = source_file.convert("RGBA")
        target_size = (source.width * 2, source.height * 2)
        result = enhanced_file.convert("RGBA").resize(target_size, Image.Resampling.LANCZOS)
        alpha = source.getchannel("A").resize(target_size, Image.Resampling.LANCZOS)
        result.putalpha(alpha)
        result.save(temporary, format="PNG")
    os.replace(temporary, output_path)


def build_receipt(
    *,
    source_prefix: str,
    source_receipt_sha256: str,
    run_id: str,
    run_attempt: int,
    max_images: int,
    assets: list[dict],
    transform: dict,
    packs: list[dict],
    map_entry: dict,
) -> dict:
    identity = parse_source_prefix(source_prefix)
    if not run_id.isdigit() or int(run_id) <= 0 or run_attempt <= 0:
        raise ValueError("derived run identity is invalid")
    candidate_prefix = (
        f"candidates/derived-images/realesrgan-x2/{identity.data_version}/runs/{run_id}-{run_attempt}"
    )
    locale_counts = {locale: sum(item.get("ownerLocale") == locale for item in assets) for locale in LOCALES}
    return {
        "schemaVersion": 1,
        "dataVersion": identity.data_version,
        "runId": run_id,
        "runAttempt": run_attempt,
        "candidatePrefix": candidate_prefix,
        "mode": "complete" if max_images == 0 else "partial",
        "maxImagesPerLocale": max_images,
        "sourceCandidate": {
            "prefix": source_prefix,
            "receiptSha256": source_receipt_sha256,
        },
        "selectedImageCount": len(assets),
        "ownerLocaleCounts": locale_counts,
        "transform": transform,
        "packs": packs,
        "map": map_entry,
    }


class R2Store:
    def __init__(self) -> None:
        try:
            import boto3
            from boto3.s3.transfer import TransferConfig
            from botocore.config import Config
        except ImportError as error:
            raise RuntimeError("boto3 dependency is required") from error
        account_id = os.environ.get("R2_ACCOUNT_ID", "")
        self.bucket = os.environ.get("R2_BUCKET", "")
        access_key = os.environ.get("R2_ACCESS_KEY_ID", "")
        secret_key = os.environ.get("R2_SECRET_ACCESS_KEY", "")
        if not all((account_id, self.bucket, access_key, secret_key)):
            raise RuntimeError("R2_ACCOUNT_ID, R2_BUCKET and R2 credentials are required")
        self.client = boto3.client(
            "s3",
            endpoint_url=f"https://{account_id}.r2.cloudflarestorage.com",
            region_name="auto",
            aws_access_key_id=access_key,
            aws_secret_access_key=secret_key,
            config=Config(
                signature_version="s3v4",
                connect_timeout=30,
                read_timeout=300,
                retries={"max_attempts": 5, "mode": "standard"},
            ),
        )
        self.transfer = TransferConfig(
            multipart_threshold=64 * 1024 * 1024,
            multipart_chunksize=64 * 1024 * 1024,
            max_concurrency=4,
            use_threads=True,
        )

    def get_bytes(self, key: str) -> bytes:
        return self.client.get_object(Bucket=self.bucket, Key=key)["Body"].read()

    def download(self, key: str, destination: Path) -> None:
        destination.parent.mkdir(parents=True, exist_ok=True)
        self.client.download_file(self.bucket, key, str(destination), Config=self.transfer)

    def upload(self, source: Path, key: str, content_type: str) -> None:
        self.client.upload_file(
            str(source),
            self.bucket,
            key,
            ExtraArgs={
                "ContentType": content_type,
                "CacheControl": "no-store",
                "Metadata": {"sha256": sha256_file(source), "bytes": str(source.stat().st_size)},
            },
            Config=self.transfer,
        )

    def put_bytes(self, value: bytes, key: str, content_type: str) -> None:
        self.client.put_object(
            Bucket=self.bucket,
            Key=key,
            Body=value,
            ContentType=content_type,
            CacheControl="no-store",
            Metadata={"sha256": sha256_bytes(value), "bytes": str(len(value))},
        )


def safe_extract_zip(archive_path: Path, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    root = destination.resolve()
    with zipfile.ZipFile(archive_path) as archive:
        for entry in archive.infolist():
            relative = PurePosixPath(entry.filename)
            if relative.is_absolute() or ".." in relative.parts:
                raise ValueError("tool archive contains unsafe path")
            target = destination.joinpath(*relative.parts).resolve()
            if root != target and root not in target.parents:
                raise ValueError("tool archive path escapes destination")
        archive.extractall(destination)


def prepare_tool(tool_cache: Path) -> tuple[Path, Path, dict]:
    tool_cache.mkdir(parents=True, exist_ok=True)
    archive_path = tool_cache / "realesrgan-ncnn-vulkan-20220424-windows.zip"
    if not archive_path.exists():
        emit("tool_download_started")
        urllib.request.urlretrieve(TOOL_URL, archive_path)
    if sha256_file(archive_path) != TOOL_ARCHIVE_SHA256:
        raise ValueError("Real-ESRGAN archive SHA-256 mismatch")
    extracted = tool_cache / "extracted"
    exe_candidates = list(extracted.rglob("realesrgan-ncnn-vulkan.exe")) if extracted.exists() else []
    if not exe_candidates:
        safe_extract_zip(archive_path, extracted)
        exe_candidates = list(extracted.rglob("realesrgan-ncnn-vulkan.exe"))
    if len(exe_candidates) != 1:
        raise ValueError("Real-ESRGAN executable layout is invalid")
    exe = exe_candidates[0]
    model_dir = exe.parent / "models"
    param = model_dir / "realesrgan-x4plus.param"
    binary = model_dir / "realesrgan-x4plus.bin"
    checks = {
        exe: TOOL_EXE_SHA256,
        param: MODEL_PARAM_SHA256,
        binary: MODEL_BIN_SHA256,
    }
    for path, expected in checks.items():
        if not path.is_file() or sha256_file(path) != expected:
            raise ValueError(f"Real-ESRGAN tool hash mismatch: {path.name}")
    return exe, model_dir, {
        "toolVersion": "0.2.5.0",
        "toolSha256": TOOL_EXE_SHA256,
        "model": "realesrgan-x4plus",
        "modelParamSha256": MODEL_PARAM_SHA256,
        "modelBinSha256": MODEL_BIN_SHA256,
    }


def download_source(
    store: R2Store, source_prefix: str, work_root: Path, max_images: int
) -> tuple[SourceIdentity, str, list[dict]]:
    identity = parse_source_prefix(source_prefix)
    source_root = work_root / "source-candidate"
    receipt_bytes = store.get_bytes(f"{source_prefix}/receipt.json")
    try:
        receipt = json.loads(receipt_bytes)
    except json.JSONDecodeError as error:
        raise ValueError("source receipt JSON is invalid") from error
    packs, maps = validate_source_receipt(receipt, source_prefix)
    (source_root / "receipt.json").parent.mkdir(parents=True, exist_ok=True)
    (source_root / "receipt.json").write_bytes(receipt_bytes)

    references: list[dict] = []
    for map_entry in maps:
        map_path = source_root.joinpath(*map_entry["path"].split("/"))
        store.download(f"{source_prefix}/{map_entry['path']}", map_path)
        verify_file_entry(source_root, map_entry)
        locale_match = MAP_PATTERN.fullmatch(map_entry["path"])
        assert locale_match is not None
        references.extend(source_normal_references(map_path, locale_match.group(1), identity.data_version))
    if not references:
        raise ValueError("source maps contain no available normal images")

    selected_hashes = select_reference_hashes(references, max_images)
    selected_references = [item for item in references if item["sourceSha256"] in selected_hashes]
    required = required_pack_entries(packs, selected_references)
    for pack_entry in required:
        pack_path = source_root.joinpath(*pack_entry["path"].split("/"))
        store.download(f"{source_prefix}/{pack_entry['path']}", pack_path)
        verify_file_entry(source_root, pack_entry)
    emit(
        "source_download_complete",
        mapCount=len(maps),
        packCount=len(required),
        selectedImageCount=len(selected_hashes),
    )
    assets = extract_selected_sources(source_root, required, selected_references, selected_hashes)
    return identity, sha256_bytes(receipt_bytes), assets


def extract_selected_sources(
    source_root: Path,
    pack_entries: list[dict],
    references: list[dict],
    selected_hashes: set[str],
) -> list[dict]:
    zstandard = _zstd_module()
    extracted_root = source_root / "selected-members"
    extracted_root.mkdir(parents=True, exist_ok=True)
    source_by_hash: dict[str, dict] = {}
    for pack_entry in pack_entries:
        pack_path = verify_file_entry(source_root, pack_entry)
        member_count = 0
        unpacked_bytes = 0
        seen_members: set[str] = set()
        with pack_path.open("rb") as compressed:
            with zstandard.ZstdDecompressor().stream_reader(compressed) as reader:
                with tarfile.open(fileobj=reader, mode="r|") as archive:
                    for member in archive:
                        if not member.isfile() or PurePosixPath(member.name).name != member.name:
                            raise ValueError("source pack contains unsafe non-file member")
                        if member.name in seen_members:
                            raise ValueError("source pack contains duplicate member")
                        seen_members.add(member.name)
                        body = archive.extractfile(member)
                        if body is None:
                            raise ValueError("source pack member body is missing")
                        payload = body.read()
                        digest, extension = validate_source_member(member.name, payload)
                        if member.size != len(payload):
                            raise ValueError("source pack member size mismatch")
                        member_count += 1
                        unpacked_bytes += len(payload)
                        if digest not in selected_hashes:
                            continue
                        if digest in source_by_hash:
                            raise ValueError("selected source image appears in multiple packs")
                        destination = extracted_root / f"{digest}.{extension}"
                        destination.write_bytes(payload)
                        source_by_hash[digest] = {
                            "sourcePack": pack_entry["path"],
                            "sourceMember": member.name,
                            "sourceSha256": digest,
                            "sourceBytes": len(payload),
                            "sourcePath": destination,
                        }
        if member_count != pack_entry["memberCount"] or unpacked_bytes != pack_entry["unpackedBytes"]:
            raise ValueError("source pack aggregate mismatch")

    if set(source_by_hash) != selected_hashes:
        raise ValueError("selected source images are missing from verified packs")
    refs_by_hash: dict[str, list[dict]] = defaultdict(list)
    for reference in references:
        source = source_by_hash.get(reference["sourceSha256"])
        if source is None:
            raise ValueError("selected normal reference has no source member")
        if (
            source["sourcePack"] != reference["sourcePack"]
            or source["sourceMember"] != reference["sourceMember"]
            or source["sourceBytes"] != reference["sourceBytes"]
        ):
            raise ValueError("selected normal reference disagrees with source pack")
        refs_by_hash[reference["sourceSha256"]].append(
            {"locale": reference["locale"], "cardId": reference["cardId"]}
        )

    Image = _pillow_image()
    assets: list[dict] = []
    for digest in sorted(selected_hashes):
        source = source_by_hash[digest]
        with Image.open(source["sourcePath"]) as image:
            image.verify()
        with Image.open(source["sourcePath"]) as image:
            width, height = image.size
        references_for_hash = sorted(
            refs_by_hash[digest], key=lambda item: (LOCALES.index(item["locale"]), item["cardId"])
        )
        owner_locale = references_for_hash[0]["locale"]
        assets.append(
            {
                **source,
                "sourceWidth": width,
                "sourceHeight": height,
                "ownerLocale": owner_locale,
                "references": references_for_hash,
            }
        )
    return sorted(assets, key=lambda item: (LOCALES.index(item["ownerLocale"]), item["sourceSha256"]))


def prepare_inference_inputs(assets: list[dict], work_root: Path) -> Path:
    Image = _pillow_image()
    input_root = work_root / "inference-input"
    for asset in assets:
        destination = input_root / asset["ownerLocale"] / f"{asset['sourceSha256']}.png"
        destination.parent.mkdir(parents=True, exist_ok=True)
        with Image.open(asset["sourcePath"]) as source:
            source.convert("RGBA").save(destination, format="PNG")
        asset["inferenceInput"] = destination
    return input_root


def run_inference(
    assets: list[dict], exe: Path, model_dir: Path, work_root: Path
) -> Path:
    input_root = prepare_inference_inputs(assets, work_root)
    x4_root = work_root / "x4-intermediate"
    log_path = work_root / "realesrgan.log"
    total = len(assets)
    completed_before = 0
    with log_path.open("a", encoding="utf-8", errors="replace") as log:
        for locale in LOCALES:
            locale_assets = [item for item in assets if item["ownerLocale"] == locale]
            if not locale_assets:
                continue
            source_dir = input_root / locale
            output_dir = x4_root / locale
            output_dir.mkdir(parents=True, exist_ok=True)
            command = [
                str(exe),
                "-i",
                str(source_dir),
                "-o",
                str(output_dir),
                "-s",
                "4",
                "-m",
                str(model_dir),
                "-n",
                "realesrgan-x4plus",
                "-g",
                "0",
                "-f",
                "png",
            ]
            process = subprocess.Popen(
                command,
                cwd=exe.parent,
                stdout=log,
                stderr=subprocess.STDOUT,
                text=True,
            )
            last_count = -1
            while process.poll() is None:
                current = len(list(output_dir.glob("*.png")))
                if current != last_count:
                    emit(
                        "inference_progress",
                        locale=locale,
                        completed=completed_before + current,
                        total=total,
                    )
                    last_count = current
                time.sleep(1)
            if process.returncode != 0:
                raise RuntimeError(f"Real-ESRGAN failed for {locale} with exit {process.returncode}")
            outputs = list(output_dir.glob("*.png"))
            if len(outputs) != len(locale_assets):
                raise RuntimeError("Real-ESRGAN output count mismatch")
            completed_before += len(outputs)
    return x4_root


def postprocess_outputs(assets: list[dict], x4_root: Path, work_root: Path) -> Path:
    Image = _pillow_image()
    x2_root = work_root / "x2-final"
    for index, asset in enumerate(assets, start=1):
        enhanced = x4_root / asset["ownerLocale"] / f"{asset['sourceSha256']}.png"
        provisional = x2_root / asset["ownerLocale"] / f"{asset['sourceSha256']}.png"
        finalize_x2(asset["sourcePath"], enhanced, provisional)
        with Image.open(provisional) as output:
            output.verify()
        with Image.open(provisional) as output:
            if output.mode != "RGBA" or output.size != (
                asset["sourceWidth"] * 2,
                asset["sourceHeight"] * 2,
            ):
                raise ValueError("postprocessed image dimensions or mode are invalid")
            output_width, output_height = output.size
        output_hash = sha256_file(provisional)
        final_path = x2_root / asset["ownerLocale"] / f"{output_hash}.png"
        if final_path != provisional:
            if final_path.exists() and sha256_file(final_path) != output_hash:
                raise ValueError("output hash path collision")
            provisional.replace(final_path)
        asset.update(
            {
                "outputPath": final_path,
                "outputSha256": output_hash,
                "outputBytes": final_path.stat().st_size,
                "outputWidth": output_width,
                "outputHeight": output_height,
            }
        )
        if index == len(assets) or index % 25 == 0:
            emit("postprocess_progress", completed=index, total=len(assets))
    return x2_root


def write_pack(pack_path: Path, members: list[dict]) -> dict:
    zstandard = _zstd_module()
    pack_path.parent.mkdir(parents=True, exist_ok=True)
    unpacked_bytes = 0
    with pack_path.open("wb") as raw:
        compressor = zstandard.ZstdCompressor(
            level=3, threads=0, write_checksum=True, write_content_size=False
        )
        with compressor.stream_writer(raw, closefd=False) as compressed:
            with tarfile.open(fileobj=compressed, mode="w|", format=tarfile.USTAR_FORMAT) as archive:
                for member in sorted(members, key=lambda item: item["outputSha256"]):
                    payload_path = member["outputPath"]
                    payload_size = payload_path.stat().st_size
                    info = tarfile.TarInfo(f"{member['outputSha256']}.png")
                    info.size = payload_size
                    info.mtime = 0
                    info.mode = 0o644
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    with payload_path.open("rb") as payload:
                        archive.addfile(info, payload)
                    unpacked_bytes += payload_size
    return {
        "path": pack_path.as_posix(),
        "bytes": pack_path.stat().st_size,
        "sha256": sha256_file(pack_path),
        "memberCount": len(members),
        "unpackedBytes": unpacked_bytes,
    }


def build_outputs(assets: list[dict], candidate_root: Path) -> tuple[list[dict], dict]:
    canonical_outputs: dict[str, dict] = {}
    for asset in assets:
        canonical_outputs.setdefault(asset["outputSha256"], asset)
    groups: dict[str, list[dict]] = {locale: [] for locale in LOCALES}
    for output in canonical_outputs.values():
        groups[output["ownerLocale"]].append(output)

    output_location: dict[str, tuple[str, str]] = {}
    pack_entries: list[dict] = []
    for locale in LOCALES:
        members = groups[locale]
        if not members:
            continue
        relative = f"packs/{locale}-normal-realesrgan-x2.tar.zst"
        receipt = write_pack(candidate_root.joinpath(*relative.split("/")), members)
        receipt["path"] = relative
        pack_entries.append(receipt)
        for member in members:
            output_location[member["outputSha256"]] = (
                relative,
                f"{member['outputSha256']}.png",
            )

    map_assets: list[dict] = []
    for asset in sorted(assets, key=lambda item: item["sourceSha256"]):
        pack, member = output_location[asset["outputSha256"]]
        map_assets.append(
            {
                "ownerLocale": asset["ownerLocale"],
                "references": asset["references"],
                "source": {
                    "pack": asset["sourcePack"],
                    "member": asset["sourceMember"],
                    "sha256": asset["sourceSha256"],
                    "bytes": asset["sourceBytes"],
                    "width": asset["sourceWidth"],
                    "height": asset["sourceHeight"],
                },
                "output": {
                    "pack": pack,
                    "member": member,
                    "sha256": asset["outputSha256"],
                    "bytes": asset["outputBytes"],
                    "width": asset["outputWidth"],
                    "height": asset["outputHeight"],
                    "mediaType": "image/png",
                },
            }
        )
    identity = {
        "schemaVersion": 1,
        "transform": "realesrgan-x4plus-lanczos-x2-original-alpha",
        "assets": map_assets,
    }
    map_bytes = _zstd_module().ZstdCompressor(
        level=3, threads=0, write_checksum=True, write_content_size=True
    ).compress(canonical_json(identity))
    relative_map = "maps/normal-realesrgan-x2.json.zst"
    map_path = candidate_root.joinpath(*relative_map.split("/"))
    map_path.parent.mkdir(parents=True, exist_ok=True)
    map_path.write_bytes(map_bytes)
    map_entry = {
        "path": relative_map,
        "bytes": len(map_bytes),
        "sha256": sha256_bytes(map_bytes),
    }
    return pack_entries, map_entry


def verify_output_pack(path: Path) -> dict[str, int]:
    zstandard = _zstd_module()
    members: dict[str, int] = {}
    with path.open("rb") as compressed:
        with zstandard.ZstdDecompressor().stream_reader(compressed) as reader:
            with tarfile.open(fileobj=reader, mode="r|") as archive:
                for info in archive:
                    if not info.isfile() or PurePosixPath(info.name).name != info.name:
                        raise ValueError("derived pack contains unsafe non-file member")
                    match = OUTPUT_MEMBER_PATTERN.fullmatch(info.name)
                    if match is None or info.name in members:
                        raise ValueError("derived pack member identity is invalid")
                    source = archive.extractfile(info)
                    if source is None:
                        raise ValueError("derived pack member body is missing")
                    payload = source.read()
                    if len(payload) != info.size or sha256_bytes(payload) != match.group(1):
                        raise ValueError("derived pack member hash or size mismatch")
                    members[info.name] = len(payload)
    return members


def validate_derived_receipt(receipt: object, candidate_root: Path | None = None) -> dict:
    if not isinstance(receipt, dict):
        raise ValueError("derived receipt is invalid")
    prefix = receipt.get("candidatePrefix", "")
    match = DERIVED_PREFIX_PATTERN.fullmatch(prefix)
    source = receipt.get("sourceCandidate")
    map_entry = receipt.get("map")
    packs = receipt.get("packs")
    if (
        receipt.get("schemaVersion") != 1
        or match is None
        or receipt.get("dataVersion") != match.group(1)
        or str(receipt.get("runId")) != match.group(2)
        or receipt.get("runAttempt") != int(match.group(3))
        or receipt.get("mode") not in {"partial", "complete"}
        or not isinstance(source, dict)
        or SOURCE_PREFIX_PATTERN.fullmatch(source.get("prefix", "")) is None
        or HASH_PATTERN.fullmatch(source.get("receiptSha256", "")) is None
        or not isinstance(packs, list)
        or not isinstance(map_entry, dict)
    ):
        raise ValueError("derived receipt identity is invalid")
    pack_pattern = re.compile(r"^packs/(ko_KR|en_US)-normal-realesrgan-x2\.tar\.zst$")
    checked_packs = [_checked_object(entry, pack_pattern, "derived pack") for entry in packs]
    checked_map = _checked_object(
        map_entry,
        re.compile(r"^maps/normal-realesrgan-x2\.json\.zst$"),
        "derived map",
    )
    paths = [entry["path"] for entry in [*checked_packs, checked_map]]
    if len(paths) != len(set(paths)):
        raise ValueError("derived receipt object paths are duplicated")
    if candidate_root is not None:
        for entry in [*checked_packs, checked_map]:
            verify_file_entry(candidate_root, entry)
    return receipt


def verify_candidate(candidate_root: Path) -> dict:
    receipt_path = candidate_root / "receipt.json"
    receipt_bytes = receipt_path.read_bytes()
    receipt = json.loads(receipt_bytes)
    if canonical_json(receipt) != receipt_bytes:
        raise ValueError("derived receipt is not canonical JSON")
    validate_derived_receipt(receipt, candidate_root)
    all_members: dict[tuple[str, str], int] = {}
    all_hashes: set[str] = set()
    for pack in receipt["packs"]:
        pack_path = verify_file_entry(candidate_root, pack)
        members = verify_output_pack(pack_path)
        if (
            len(members) != pack.get("memberCount")
            or sum(members.values()) != pack.get("unpackedBytes")
        ):
            raise ValueError("derived pack aggregate mismatch")
        for member, byte_count in members.items():
            digest = OUTPUT_MEMBER_PATTERN.fullmatch(member).group(1)
            if digest in all_hashes:
                raise ValueError("derived output hash appears in multiple packs")
            all_hashes.add(digest)
            all_members[(pack["path"], member)] = byte_count
    map_path = verify_file_entry(candidate_root, receipt["map"])
    map_value = read_zstd_json(map_path)
    decoded = _zstd_module().ZstdDecompressor().decompress(map_path.read_bytes())
    if canonical_json(map_value) != decoded:
        raise ValueError("derived map is not canonical JSON")
    if (
        map_value.get("schemaVersion") != 1
        or map_value.get("transform") != "realesrgan-x4plus-lanczos-x2-original-alpha"
        or not isinstance(map_value.get("assets"), list)
        or len(map_value["assets"]) != receipt.get("selectedImageCount")
    ):
        raise ValueError("derived map identity is invalid")
    previous_source = ""
    referenced_outputs: set[str] = set()
    for asset in map_value["assets"]:
        source = asset.get("source", {})
        output = asset.get("output", {})
        source_hash = source.get("sha256", "")
        if HASH_PATTERN.fullmatch(source_hash) is None or source_hash <= previous_source:
            raise ValueError("derived map source hashes are not strictly ordered")
        previous_source = source_hash
        output_hash = output.get("sha256", "")
        location = (output.get("pack"), output.get("member"))
        if (
            HASH_PATTERN.fullmatch(output_hash) is None
            or output.get("member") != f"{output_hash}.png"
            or all_members.get(location) != output.get("bytes")
            or output.get("width") != source.get("width", 0) * 2
            or output.get("height") != source.get("height", 0) * 2
            or output.get("mediaType") != "image/png"
        ):
            raise ValueError("derived map output reference is invalid")
        referenced_outputs.add(output_hash)
    if referenced_outputs != all_hashes:
        raise ValueError("derived map and pack output sets differ")
    emit(
        "candidate_verified",
        selectedImageCount=receipt["selectedImageCount"],
        uniqueOutputCount=len(all_hashes),
    )
    return receipt


def dependency_versions() -> dict:
    import boto3
    import PIL
    import zstandard

    return {
        "python": ".".join(str(part) for part in sys.version_info[:3]),
        "boto3": boto3.__version__,
        "pillow": PIL.__version__,
        "zstandard": zstandard.__version__,
    }


def gpu_identity() -> dict:
    result = subprocess.run(
        [
            "nvidia-smi",
            "--query-gpu=name,driver_version",
            "--format=csv,noheader",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    line = result.stdout.strip().splitlines()
    if len(line) != 1 or "," not in line[0]:
        raise RuntimeError("expected exactly one NVIDIA GPU")
    name, driver = (part.strip() for part in line[0].split(",", 1))
    return {"gpu": name, "driver": driver}


def build_candidate(args: argparse.Namespace) -> Path:
    source_prefix = args.source_prefix
    identity = parse_source_prefix(source_prefix)
    if args.max_images < 0:
        raise ValueError("max_images must be zero or positive")
    output_root = args.output_root.resolve()
    work_root = args.work_root.resolve()
    candidate_prefix = (
        f"candidates/derived-images/realesrgan-x2/{identity.data_version}/"
        f"runs/{args.run_id}-{args.run_attempt}"
    )
    candidate_root = output_root.joinpath(*candidate_prefix.split("/"))
    if candidate_root.exists():
        raise ValueError("derived candidate root already exists")
    candidate_root.mkdir(parents=True)
    work_root.mkdir(parents=True, exist_ok=True)
    emit("build_started", mode="complete" if args.max_images == 0 else "partial")

    store = R2Store()
    _, source_receipt_hash, assets = download_source(
        store, source_prefix, work_root, args.max_images
    )
    exe, model_dir, tool_identity = prepare_tool(args.tool_cache.resolve())
    transform = {
        **tool_identity,
        "inferenceScale": 4,
        "finalScale": 2,
        "downsample": "Pillow Lanczos",
        "alphaPolicy": "original-alpha-lanczos-x2",
        "dependencies": dependency_versions(),
        "runner": gpu_identity(),
    }
    x4_root = run_inference(assets, exe, model_dir, work_root)
    postprocess_outputs(assets, x4_root, work_root)
    pack_entries, map_entry = build_outputs(assets, candidate_root)
    receipt = build_receipt(
        source_prefix=source_prefix,
        source_receipt_sha256=source_receipt_hash,
        run_id=args.run_id,
        run_attempt=args.run_attempt,
        max_images=args.max_images,
        assets=assets,
        transform=transform,
        packs=pack_entries,
        map_entry=map_entry,
    )
    (candidate_root / "receipt.json").write_bytes(canonical_json(receipt))
    verify_candidate(candidate_root)
    emit("build_complete", candidatePrefix=candidate_prefix, candidateRoot=str(candidate_root))
    return candidate_root


def candidate_objects(receipt: dict) -> list[dict]:
    return [*receipt["packs"], receipt["map"]]


def load_candidate(candidate_root: Path) -> tuple[dict, bytes]:
    receipt_bytes = (candidate_root / "receipt.json").read_bytes()
    receipt = json.loads(receipt_bytes)
    validate_derived_receipt(receipt, candidate_root)
    return receipt, receipt_bytes


def upload_objects(candidate_root: Path) -> None:
    receipt, _ = load_candidate(candidate_root)
    store = R2Store()
    for entry in candidate_objects(receipt):
        source = verify_file_entry(candidate_root, entry)
        store.upload(source, f"{receipt['candidatePrefix']}/{entry['path']}", "application/zstd")
        emit("object_uploaded", path=entry["path"], bytes=entry["bytes"])


def verify_remote(candidate_root: Path, download_root: Path) -> None:
    receipt, receipt_bytes = load_candidate(candidate_root)
    if download_root.exists():
        raise ValueError("remote verification root already exists")
    download_root.mkdir(parents=True)
    store = R2Store()
    for entry in candidate_objects(receipt):
        destination = download_root.joinpath(*entry["path"].split("/"))
        store.download(f"{receipt['candidatePrefix']}/{entry['path']}", destination)
    (download_root / "receipt.json").write_bytes(receipt_bytes)
    verify_candidate(download_root)
    emit("remote_objects_verified", objectCount=len(candidate_objects(receipt)))


def upload_receipt(candidate_root: Path) -> None:
    receipt, receipt_bytes = load_candidate(candidate_root)
    store = R2Store()
    key = f"{receipt['candidatePrefix']}/receipt.json"
    store.put_bytes(receipt_bytes, key, "application/json")
    remote = store.get_bytes(key)
    if remote != receipt_bytes:
        raise ValueError("remote receipt exact-byte verification failed")
    emit("receipt_uploaded_last", bytes=len(receipt_bytes), sha256=sha256_bytes(receipt_bytes))


def create_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)

    build = commands.add_parser("build")
    build.add_argument("--source-prefix", required=True)
    build.add_argument("--max-images", type=int, required=True)
    build.add_argument("--run-id", required=True)
    build.add_argument("--run-attempt", type=int, required=True)
    build.add_argument("--output-root", type=Path, required=True)
    build.add_argument("--work-root", type=Path, required=True)
    build.add_argument("--tool-cache", type=Path, required=True)

    for name in ("verify", "upload-objects", "upload-receipt"):
        command = commands.add_parser(name)
        command.add_argument("--candidate-root", type=Path, required=True)
    remote = commands.add_parser("verify-remote")
    remote.add_argument("--candidate-root", type=Path, required=True)
    remote.add_argument("--download-root", type=Path, required=True)
    return parser


def main() -> int:
    args = create_parser().parse_args()
    try:
        if args.command == "build":
            build_candidate(args)
        elif args.command == "verify":
            verify_candidate(args.candidate_root.resolve())
        elif args.command == "upload-objects":
            upload_objects(args.candidate_root.resolve())
        elif args.command == "verify-remote":
            verify_remote(args.candidate_root.resolve(), args.download_root.resolve())
        elif args.command == "upload-receipt":
            upload_receipt(args.candidate_root.resolve())
        else:
            raise ValueError("unknown command")
        return 0
    except Exception as error:
        emit("upscale_error", errorType=type(error).__name__, message=str(error))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
