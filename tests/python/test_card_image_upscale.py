from __future__ import annotations

import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "card_image_upscale.py"


def load_module():
    spec = importlib.util.spec_from_file_location("card_image_upscale", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load card image upscale module")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def source_receipt(prefix: str) -> dict:
    return {
        "schemaVersion": 1,
        "dataVersion": "36.0.3-build247416-r1",
        "runId": "31066060504",
        "runAttempt": 1,
        "candidatePrefix": prefix,
        "inputManifestSha256": "a" * 64,
        "normalizedSha256": {"ko_KR": "b" * 64, "en_US": "c" * 64},
        "requestCount": 8,
        "absentCount": 0,
        "unavailableCount": 0,
        "successCount": 8,
        "uniqueImageCount": 6,
        "mediaTypeCounts": {"image/png": 8},
        "totalSourceBytes": 60,
        "packs": [
            {
                "path": "packs/ko_KR-normal-000.tar.zst",
                "bytes": 10,
                "sha256": "d" * 64,
                "memberCount": 2,
                "unpackedBytes": 8,
            },
            {
                "path": "packs/ko_KR-crop-000.tar.zst",
                "bytes": 10,
                "sha256": "e" * 64,
                "memberCount": 2,
                "unpackedBytes": 8,
            },
            {
                "path": "packs/en_US-normal-000.tar.zst",
                "bytes": 10,
                "sha256": "f" * 64,
                "memberCount": 2,
                "unpackedBytes": 8,
            },
        ],
        "maps": [
            {"path": "maps/ko_KR.json.zst", "bytes": 4, "sha256": "1" * 64},
            {"path": "maps/en_US.json.zst", "bytes": 4, "sha256": "2" * 64},
        ],
    }


class ContractTests(unittest.TestCase):
    def test_source_prefix_and_receipt_accept_only_completed_hcl_015_identity(self):
        module = load_module()
        prefix = "candidates/images/36.0.3-build247416-r1/runs/31066060504-1"

        identity = module.parse_source_prefix(prefix)
        packs, maps = module.validate_source_receipt(source_receipt(prefix), prefix)

        self.assertEqual(identity.data_version, "36.0.3-build247416-r1")
        self.assertEqual(identity.run_id, "31066060504")
        self.assertEqual(identity.run_attempt, 1)
        self.assertEqual(
            [entry["path"] for entry in packs],
            [
                "packs/ko_KR-normal-000.tar.zst",
                "packs/ko_KR-crop-000.tar.zst",
                "packs/en_US-normal-000.tar.zst",
            ],
        )
        self.assertEqual(len(maps), 2)

        for invalid in [
            "candidates/images/36.0.3-build247416-r1/runs/../receipt",
            "candidates/raw/36.0.3-build247416-r1/runs/1-1",
            "candidates/images/latest/runs/1-1",
        ]:
            with self.assertRaises(ValueError):
                module.parse_source_prefix(invalid)

        wrong = source_receipt(prefix)
        wrong["candidatePrefix"] = prefix + "-other"
        with self.assertRaises(ValueError):
            module.validate_source_receipt(wrong, prefix)

    def test_normal_map_references_can_require_a_crop_owner_pack(self):
        module = load_module()
        prefix = "candidates/images/36.0.3-build247416-r1/runs/31066060504-1"
        packs, _ = module.validate_source_receipt(source_receipt(prefix), prefix)
        references = [
            {
                "locale": "ko_KR",
                "cardId": 7,
                "sourceSha256": "a" * 64,
                "sourcePack": "packs/ko_KR-crop-000.tar.zst",
                "sourceMember": f"{'a' * 64}.png",
                "sourceBytes": 10,
            }
        ]

        required = module.required_pack_entries(packs, references)

        self.assertEqual([entry["path"] for entry in required], ["packs/ko_KR-crop-000.tar.zst"])

    def test_selection_is_locale_bounded_and_hash_sorted(self):
        module = load_module()
        assets = [
            {"ownerLocale": "ko_KR", "sourceSha256": "c" * 64},
            {"ownerLocale": "en_US", "sourceSha256": "4" * 64},
            {"ownerLocale": "ko_KR", "sourceSha256": "a" * 64},
            {"ownerLocale": "en_US", "sourceSha256": "2" * 64},
            {"ownerLocale": "ko_KR", "sourceSha256": "b" * 64},
        ]

        selected = module.select_assets(assets, max_images=2)

        self.assertEqual(
            [(item["ownerLocale"], item["sourceSha256"][0]) for item in selected],
            [("ko_KR", "a"), ("ko_KR", "b"), ("en_US", "2"), ("en_US", "4")],
        )
        self.assertEqual(module.select_assets(assets, max_images=0), sorted(
            assets, key=lambda item: (module.LOCALES.index(item["ownerLocale"]), item["sourceSha256"])
        ))
        with self.assertRaises(ValueError):
            module.select_assets(assets, max_images=-1)

        references = [
            {"locale": "ko_KR", "sourceSha256": "c" * 64},
            {"locale": "ko_KR", "sourceSha256": "a" * 64},
            {"locale": "en_US", "sourceSha256": "4" * 64},
            {"locale": "en_US", "sourceSha256": "2" * 64},
        ]
        self.assertEqual(
            module.select_reference_hashes(references, max_images=1),
            {"a" * 64, "2" * 64},
        )

    def test_member_validation_rejects_paths_and_hash_mismatch(self):
        module = load_module()
        payload = b"official-image"
        sha = digest(payload)

        module.validate_source_member(f"{sha}.png", payload)

        for member in [f"folder/{sha}.png", "../escape.png", "not-a-hash.png"]:
            with self.assertRaises(ValueError):
                module.validate_source_member(member, payload)
        with self.assertRaises(ValueError):
            module.validate_source_member(f"{'0' * 64}.png", payload)

    def test_receipt_is_canonical_separate_and_marks_partial_smoke(self):
        module = load_module()
        source_prefix = "candidates/images/36.0.3-build247416-r1/runs/31066060504-1"
        receipt = module.build_receipt(
            source_prefix=source_prefix,
            source_receipt_sha256="a" * 64,
            run_id="42",
            run_attempt=3,
            max_images=10,
            assets=[{"ownerLocale": "ko_KR"}, {"ownerLocale": "en_US"}],
            transform={"model": "realesrgan-x4plus"},
            packs=[],
            map_entry={"path": "maps/normal-realesrgan-x2.json.zst", "bytes": 1, "sha256": "b" * 64},
        )
        encoded = module.canonical_json(receipt)

        self.assertEqual(receipt["mode"], "partial")
        self.assertEqual(
            receipt["candidatePrefix"],
            "candidates/derived-images/realesrgan-x2/36.0.3-build247416-r1/runs/42-3",
        )
        self.assertEqual(encoded, json.dumps(receipt, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode() + b"\n")
        self.assertNotIn("createdAt", receipt)
        self.assertNotIn("C:\\", encoded.decode())

    def test_x2_postprocess_restores_resampled_original_alpha(self):
        try:
            from PIL import Image
        except ImportError:
            self.skipTest("Pillow is not installed")
        module = load_module()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source.png"
            enhanced = root / "enhanced.png"
            output = root / "output.png"
            original = Image.new("RGBA", (2, 2), (10, 20, 30, 255))
            original.putalpha(Image.new("L", (2, 2)))
            original.getchannel("A").putpixel((1, 1), 255)
            original.save(source)
            Image.new("RGB", (8, 8), (200, 100, 50)).save(enhanced)

            module.finalize_x2(source, enhanced, output)

            with Image.open(source) as source_image, Image.open(output) as result:
                expected_alpha = source_image.convert("RGBA").getchannel("A").resize(
                    (4, 4), Image.Resampling.LANCZOS
                )
                self.assertEqual(result.mode, "RGBA")
                self.assertEqual(result.size, (4, 4))
                self.assertEqual(result.getchannel("A").tobytes(), expected_alpha.tobytes())

    def test_derived_pack_map_and_receipt_verify_as_one_candidate(self):
        try:
            import zstandard  # noqa: F401
            from PIL import Image
        except ImportError:
            self.skipTest("Pillow and zstandard are required")
        module = load_module()
        source_prefix = "candidates/images/36.0.3-build247416-r1/runs/31066060504-1"
        with tempfile.TemporaryDirectory() as temporary:
            candidate_root = Path(temporary) / "candidate"
            candidate_root.mkdir()
            assets = []
            for index, locale in enumerate(module.LOCALES, start=1):
                output_path = Path(temporary) / f"output-{index}.png"
                Image.new("RGBA", (4, 6), (index * 60, 20, 30, 255)).save(output_path)
                output_sha = module.sha256_file(output_path)
                assets.append(
                    {
                        "ownerLocale": locale,
                        "references": [{"locale": locale, "cardId": index}],
                        "sourcePack": f"packs/{locale}-normal-000.tar.zst",
                        "sourceMember": f"{'a' * 63}{index}.png",
                        "sourceSha256": f"{'a' * 63}{index}",
                        "sourceBytes": 50,
                        "sourceWidth": 2,
                        "sourceHeight": 3,
                        "outputPath": output_path,
                        "outputSha256": output_sha,
                        "outputBytes": output_path.stat().st_size,
                        "outputWidth": 4,
                        "outputHeight": 6,
                    }
                )

            packs, map_entry = module.build_outputs(assets, candidate_root)
            receipt = module.build_receipt(
                source_prefix=source_prefix,
                source_receipt_sha256="b" * 64,
                run_id="42",
                run_attempt=1,
                max_images=1,
                assets=assets,
                transform={"model": "realesrgan-x4plus"},
                packs=packs,
                map_entry=map_entry,
            )
            (candidate_root / "receipt.json").write_bytes(module.canonical_json(receipt))

            verified = module.verify_candidate(candidate_root)

            self.assertEqual(verified["selectedImageCount"], 2)
            self.assertEqual(len(verified["packs"]), 2)
            pack_path = candidate_root / verified["packs"][0]["path"]
            pack_path.write_bytes(pack_path.read_bytes() + b"tamper")
            with self.assertRaises(ValueError):
                module.verify_candidate(candidate_root)


if __name__ == "__main__":
    unittest.main()
