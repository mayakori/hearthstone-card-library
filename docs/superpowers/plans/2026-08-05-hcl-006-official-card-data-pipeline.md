# HCL-006 Official Card Data Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust CLI that collects current Standard cards and metadata from Blizzard for `ko_KR` and `en_US`, preserves canonical Raw snapshots, creates deterministic locale SQLite databases, and atomically emits four zstd assets plus a manifest.

**Architecture:** A root Cargo workspace contains a dependency-light `card-data-contract` crate for shared types, canonical serialization and validation, and a non-Tauri `card-data-pipeline` binary crate for OAuth, HTTP collection, normalization, SQLite, compression and packaging. The pipeline keeps network, normalization, storage and packaging behind focused modules; the packaged Tauri app remains a consumer and does not link the pipeline crate.

**Tech Stack:** Rust 2021 on rustc 1.85+, Cargo workspace, serde 1.0, serde_json 1.0, reqwest 0.13 with rustls, tokio 1.53, clap 4.6, rusqlite 0.40 with bundled SQLite, zstd 0.13, SHA-256, secrecy 0.10, time 0.3, wiremock 0.6, assert_cmd 2.2

## Global Constraints

- Implement the approved contract in `docs/superpowers/specs/2026-07-25-hcl-006-official-card-data-pipeline-design.md`; do not reopen its locked product decisions.
- The only upstream card-data source is Blizzard Hearthstone Game Data API in region `us`.
- Collect both `ko_KR` and `en_US`; either locale failing prevents the complete package from being published.
- The fixed list query is `set=standard`, `gameMode=constructed`, `collectible=0,1`, `pageSize=500`, with one-based sequential pages.
- Access tokens, client ID, client secret and Authorization headers exist only in process memory and never appear in artifacts, fixtures or logs.
- Preserve canonical Raw JSON separately from one complete 13-table `STRICT` SQLite database per locale.
- Preserve non-collectible related cards and class references as ordinary card rows with `scope_kind`; do not merge different official IDs.
- Preserve `childIds`, `bundledCardIds`, `parentId` and `copyOfCardId` provenance and order. Only `child` and `bundled` targets are required to exist in the final card set.
- Store official text as `text_markup` and derive `text_plain` with the exact five-step transform in the spec.
- Keep rune and sideboard data as grouped nullable columns on `cards`; do not introduce one-to-one tables for them.
- Do not add FTS, `name_choseong`, image downloads, diff generation, R2 upload, signing, Tauri updater, IPC or frontend work.
- First constants are `schemaVersion = 1` and `minimumAppVersion = "0.1.0"`; manifest schema version, SQLite metadata schema version and `PRAGMA user_version` must match.
- Use JSONL on stderr and leave stdout empty. Do not log every successful HTTP request.
- Package creation is all-or-nothing and never overwrites an existing data-version directory.
- All production behavior is test-driven. Offline tests are the default; the credentialed live smoke test is ignored and explicit.
- `docs/TODO.md`, `docs/DONE.md` and `docs/kanban.html` remain main-worktree-only files.

## Planned File Structure

```text
Cargo.toml                              # workspace membership and shared profiles
Cargo.lock                              # one lockfile for all Rust workspace members
package.json                            # workspace-wide Rust check/test commands
crates/card-data-contract/
├─ Cargo.toml
└─ src/
   ├─ lib.rs                            # public modules and version constants
   ├─ version.rs                        # DataVersion parse/format
   ├─ raw.rs                            # Raw envelope and canonical JSON
   ├─ official.rs                       # typed Blizzard response fields
   ├─ normalized.rs                     # normalized rows and relation enums
   ├─ manifest.rs                       # manifest types and cross-asset validation
   └─ text.rs                           # markup-to-plain conversion
crates/card-data-pipeline/
├─ Cargo.toml
├─ src/
│  ├─ lib.rs                            # library entry point and module exports
│  ├─ main.rs                           # CLI process and exit-code mapping
│  ├─ config.rs                         # environment and CLI validation
│  ├─ error.rs                          # stable error taxonomy and exit codes
│  ├─ logging.rs                        # secret-safe JSONL events
│  ├─ oauth.rs                          # in-memory token lifecycle
│  ├─ http.rs                           # request, timeout and retry adapter
│  ├─ collect.rs                        # pagination, relation closure and locale parity
│  ├─ normalize.rs                      # API-to-normalized model mapping
│  ├─ schema.sql                        # exact 13-table DDL
│  ├─ sqlite.rs                         # deterministic SQLite writer and checks
│  ├─ package.rs                        # zstd, hashes, manifest and atomic rename
│  └─ clock.rs                          # injected UTC clock for deterministic tests
└─ tests/
   ├─ collection.rs                    # wiremock OAuth/API behavior
   ├─ normalize_sqlite.rs               # logical SQLite assertions
   ├─ package.rs                        # determinism and fault cleanup
   ├─ cli.rs                            # exit codes and redaction
   └─ live_smoke.rs                     # ignored production-path smoke test
data/fixtures/card-data-pipeline/v1/
├─ ko_KR/                               # list page, metadata and individual-card JSON
└─ en_US/                               # structurally identical localized responses
README.md                               # local build and live-smoke instructions
```

`src-tauri/Cargo.lock` is removed after the root workspace is introduced; the new root `Cargo.lock` becomes the single dependency lock. `src-tauri` remains a workspace member, but neither `src-tauri` nor the frontend depends on `card-data-pipeline`.

---

### Task 1: Establish the Rust workspace and contract primitives

**Files:**
- Create: `Cargo.toml`
- Create: `crates/card-data-contract/Cargo.toml`
- Create: `crates/card-data-contract/src/lib.rs`
- Create: `crates/card-data-contract/src/version.rs`
- Create: `crates/card-data-pipeline/Cargo.toml`
- Create: `crates/card-data-pipeline/src/lib.rs`
- Create: `crates/card-data-pipeline/src/main.rs`
- Modify: `package.json`
- Delete: `src-tauri/Cargo.lock`
- Create: `Cargo.lock` through Cargo

**Interfaces:**
- Consumes: existing `src-tauri` package and approved version regex.
- Produces: workspace members, `SCHEMA_VERSION`, `MINIMUM_APP_VERSION`, `SUPPORTED_LOCALES`, and `DataVersion::parse` for all later tasks.

- [ ] **Step 1: Write failing version contract tests**

Create `crates/card-data-contract/src/version.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::DataVersion;

    #[test]
    fn parses_the_approved_data_version() {
        let version = DataVersion::parse("36.0.3-build247416-r1").unwrap();
        assert_eq!(version.official_patch_version(), "36.0.3");
        assert_eq!(version.build_id(), 247_416);
        assert_eq!(version.revision(), 1);
        assert_eq!(version.to_string(), "36.0.3-build247416-r1");
    }

    #[test]
    fn rejects_zero_and_unstructured_versions() {
        for value in ["36.0.3-build0-r1", "36.0.3-build1-r0", "latest"] {
            assert!(DataVersion::parse(value).is_err(), "accepted {value}");
        }
    }
}
```

- [ ] **Step 2: Create the workspace skeleton and verify RED**

Use this root workspace:

```toml
[workspace]
members = ["crates/card-data-contract", "crates/card-data-pipeline", "src-tauri"]
resolver = "2"
```

Use package names `card-data-contract` and `card-data-pipeline`; the pipeline binary name is `card-data-pipeline`. Run:

```powershell
cargo test -p card-data-contract version::tests --no-fail-fast
```

Expected: FAIL because `DataVersion` and constants are not implemented.

- [ ] **Step 3: Implement exact version primitives**

Export these values from `lib.rs`:

```rust
pub const SCHEMA_VERSION: u32 = 1;
pub const MINIMUM_APP_VERSION: &str = "0.1.0";
pub const SUPPORTED_LOCALES: [&str; 2] = ["ko_KR", "en_US"];
```

Implement `DataVersion` as parsed numeric patch components, positive `build_id`, positive `revision`, and canonical `Display`. Reject values that do not match `^[0-9]+\.[0-9]+\.[0-9]+-build[1-9][0-9]*-r[1-9][0-9]*$`.

- [ ] **Step 4: Make workspace verification commands authoritative**

Change scripts to:

```json
"rust:test": "cargo test --workspace",
"rust:check": "cargo check --workspace"
```

Include `npm run rust:test` in `npm run check` before `npm run rust:check`. Generate the root lockfile with:

```powershell
cargo generate-lockfile
```

- [ ] **Step 5: Verify GREEN and commit**

```powershell
cargo test -p card-data-contract version::tests --no-fail-fast
cargo check --workspace
git diff --check
git add -- Cargo.toml Cargo.lock package.json crates/card-data-contract crates/card-data-pipeline src-tauri/Cargo.lock
git commit -m "feat(HCL-006): establish card data workspace"
```

Expected: version tests pass and all three workspace members compile.

---

### Task 2: Define official, Raw, normalized and manifest contracts

**Files:**
- Modify: `crates/card-data-contract/Cargo.toml`
- Modify: `crates/card-data-contract/src/lib.rs`
- Create: `crates/card-data-contract/src/official.rs`
- Create: `crates/card-data-contract/src/raw.rs`
- Create: `crates/card-data-contract/src/normalized.rs`
- Create: `crates/card-data-contract/src/manifest.rs`
- Create: `crates/card-data-contract/src/text.rs`

**Interfaces:**
- Consumes: Task 1 constants and `DataVersion`.
- Produces: `OfficialCard`, `MetadataResponse`, `RawSnapshot`, `canonical_json_bytes`, `NormalizedCatalog`, `CardRelation`, `Manifest`, `AssetDescriptor`, `plain_text`, and contract validators.

- [ ] **Step 1: Write failing canonical JSON and text tests**

Add tests that assert exact bytes and the approved text transform:

```rust
#[test]
fn canonical_json_sorts_response_objects_but_preserves_arrays() {
    let value = serde_json::json!({"z": 1, "a": {"y": 2, "b": 3}, "items": [2, 1]});
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
```

Run `cargo test -p card-data-contract` and confirm RED because the modules do not exist.

- [ ] **Step 2: Implement typed official response boundaries**

Add contract dependencies `serde = "1.0"` with derive, `serde_json = "1.0"`, `thiserror = "2.0"`, `regex = "1.13"`, `html-escape = "0.2"`, `sha2 = "0.11"` and `hex = "0.4"`.

Define `OfficialCard` fields using the exact Blizzard keys observed by the spec: required IDs, slug, collectible, mana cost and typed optional values; vectors default to empty; `copyOfCardId` remains a vector; `runeCost` and `sideboard` are typed objects. Keep `#[serde(flatten)] extra: BTreeMap<String, Value>` so typed normalization can reject wrong known types while Raw still preserves future fields.

Define `CardsPageResponse { cards, card_count, page_count, page }` and `MetadataResponse` with the seven required arrays. Metadata also has flattened extra top-level fields.

- [ ] **Step 3: Implement the exact Raw envelope**

Use declaration order matching the spec:

```rust
pub struct RawSnapshot {
    pub format_version: u32,
    pub source: RawSource,
    pub collected_at: String,
    pub query: CardListQuery,
    pub card_pages: Vec<RawPage>,
    pub related_cards: Vec<RequestedCardResponse>,
    pub class_reference_cards: Vec<RequestedCardResponse>,
    pub metadata: RawMetadata,
}
```

`canonical_json_bytes` recursively rebuilds every `Value::Object` in lexical key order, preserves array order, serializes compact UTF-8, and appends exactly one LF. Add validation for fixed endpoints, fixed query, ascending page/request IDs and forbidden secret-shaped fields.

- [ ] **Step 4: Implement normalized and manifest value types**

`NormalizedCatalog` owns locale, generated time, source Raw hash, taxonomy rows, cards, ordered joins and relations. `RelationKind` serializes as `child`, `bundled`, `parent`, `copy_of`; `SourceField` serializes as the four official JSON field names and validates the allowed pair.

`Manifest::validate` must check:

```text
schemaVersion == 1
minimumAppVersion == 0.1.0
supportedLocales == [ko_KR, en_US]
locales keys == supportedLocales
every SHA-256 == exactly 64 lowercase hex characters
each total count == standard + related + class_reference
raw.defaultDownload == false
normalized.defaultDownload == true
```

- [ ] **Step 5: Implement the exact text transform and GREEN tests**

Apply the spec order: newline normalization, case-insensitive `<br\s*/?>` replacement, remaining tag removal, HTML entity decode, outer trim. Do not collapse internal whitespace. Run:

```powershell
cargo test -p card-data-contract --no-fail-fast
cargo clippy -p card-data-contract --all-targets -- -D warnings
git add -- crates/card-data-contract
git commit -m "feat(HCL-006): define card data contracts"
```

---

### Task 3: Add a canonical two-locale offline fixture

**Files:**
- Create: `data/fixtures/card-data-pipeline/v1/ko_KR/cards-page-1.json`
- Create: `data/fixtures/card-data-pipeline/v1/ko_KR/metadata.json`
- Create: `data/fixtures/card-data-pipeline/v1/ko_KR/cards/2001.json`
- Create: `data/fixtures/card-data-pipeline/v1/ko_KR/cards/2002.json`
- Create: `data/fixtures/card-data-pipeline/v1/ko_KR/cards/2003.json`
- Create: `data/fixtures/card-data-pipeline/v1/ko_KR/cards/3001.json`
- Create: `data/fixtures/card-data-pipeline/v1/ko_KR/cards/3002.json`
- Create: `data/fixtures/card-data-pipeline/v1/en_US/cards-page-1.json`
- Create: `data/fixtures/card-data-pipeline/v1/en_US/metadata.json`
- Create: `data/fixtures/card-data-pipeline/v1/en_US/cards/2001.json`
- Create: `data/fixtures/card-data-pipeline/v1/en_US/cards/2002.json`
- Create: `data/fixtures/card-data-pipeline/v1/en_US/cards/2003.json`
- Create: `data/fixtures/card-data-pipeline/v1/en_US/cards/3001.json`
- Create: `data/fixtures/card-data-pipeline/v1/en_US/cards/3002.json`
- Create: `crates/card-data-pipeline/tests/fixture_contract.rs`

**Interfaces:**
- Consumes: Task 2 official response types.
- Produces: a small offline input with all normalization cases; every later integration test mutates this fixture in memory instead of copying failure fixtures.

- [ ] **Step 1: Write a failing fixture coverage test**

The test loads both locale trees and asserts this exact coverage matrix:

```rust
assert_eq!(ko.card_ids(), en.card_ids());
assert!(ko.card(1001).child_ids.contains(&2001));
assert!(ko.card(1001).bundled_card_ids.contains(&2002));
assert!(ko.card(2001).child_ids.contains(&2003));
assert_eq!(ko.card(1001).parent_id, Some(9999));
assert_eq!(ko.card(1001).copy_of_card_ids, vec![9998]);
assert_eq!(ko.metadata.class(1).card_id, Some(3001));
assert_eq!(ko.metadata.class(1).hero_power_card_id, Some(3002));
assert!(ko.cards().any(|card| card.rune_cost.is_some()));
assert!(ko.cards().any(|card| card.sideboard.is_some()));
```

Run the test and confirm RED because the fixture files do not exist.

- [ ] **Step 2: Create the list and metadata responses**

Use artificial positive IDs reserved for tests. List page 1 contains Standard IDs `1001` through `1007`; `cardCount=7`, `pageCount=1`, `page=1`. It must include a normal minion, spell, location, weapon, rune card, sideboard card and multi-class/multi-type card. Use metadata IDs referenced by those cards and include all seven required arrays.

- [ ] **Step 3: Create closure and class-reference responses**

IDs `2001`, `2002`, `2003` are non-collectible related cards. `2001` points forward to `2003` to prove recursive closure. IDs `3001`, `3002` are the class hero and hero power. IDs `9998`, `9999` are deliberately absent to prove that `copy_of` and `parent` targets remain dangling without FK enforcement.

- [ ] **Step 4: Localize without changing structure**

The two locale trees must differ only in localized `name`, `text`, `flavorText`, `artistName`, taxonomy text and image URL strings. Include one empty Korean localized string and one English fallback-looking Korean value; neither is a fixture failure.

- [ ] **Step 5: Verify fixture coverage and commit**

```powershell
cargo test -p card-data-pipeline --test fixture_contract --no-fail-fast
git diff --check
git add -- data/fixtures/card-data-pipeline crates/card-data-pipeline/tests/fixture_contract.rs
git commit -m "test(HCL-006): add canonical API fixture"
```

---

### Task 4: Implement OAuth, HTTP retry, pagination and relation closure

**Files:**
- Modify: `crates/card-data-pipeline/Cargo.toml`
- Create: `crates/card-data-pipeline/src/config.rs`
- Create: `crates/card-data-pipeline/src/error.rs`
- Create: `crates/card-data-pipeline/src/clock.rs`
- Create: `crates/card-data-pipeline/src/oauth.rs`
- Create: `crates/card-data-pipeline/src/http.rs`
- Create: `crates/card-data-pipeline/src/collect.rs`
- Modify: `crates/card-data-pipeline/src/lib.rs`
- Create: `crates/card-data-pipeline/tests/collection.rs`

**Interfaces:**
- Consumes: official/Raw contract types and Task 3 fixture.
- Produces: `Credentials`, `HttpPolicy`, `TokenProvider`, `BlizzardClient`, `Collector::collect_all`, `CollectedLocales`, and stable `PipelineError` variants.

- [ ] **Step 1: Write failing wiremock tests for OAuth and retry**

Cover these exact sequences:

```text
token success -> list success
list 401 -> one new token -> same request success
list 401 -> refresh -> list 401 -> Auth error
429 with Retry-After -> success
500 -> 502 -> 503 -> 504 -> Network error after three retries
connect/read timeout -> three retries -> Network error
```

Inject a fake sleeper so tests record delta-seconds and HTTP-date `Retry-After`, or `1s, 2s, 4s + bounded jitter`, without wall-clock sleeping. Verify no recorded event includes either fixture credential or returned token.

- [ ] **Step 2: Implement config and stable errors**

Add pipeline dependencies `reqwest = "0.13.4"` with default features disabled and `json`, `form`, `query`, `rustls`; `tokio = "1.53"` with `macros`, `rt-multi-thread`, `time` and `fs`; `secrecy = "0.10"`; `time = "0.3"` with formatting/parsing; `rand = "0.10"`; `httpdate = "1"`; `async-trait = "0.1"`; and dev dependency `wiremock = "0.6"`.

Use:

```rust
pub struct Credentials {
    pub client_id: secrecy::SecretString,
    pub client_secret: secrecy::SecretString,
}

pub struct HttpPolicy {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub max_retries: u8,
}
```

Defaults are 10 seconds, 60 seconds and 3 retries. `PipelineError` separates CLI/config, auth, network, API structure, normalization/SQLite and package I/O so Task 7 can map exit codes without string matching.

- [ ] **Step 3: Implement in-memory token lifecycle**

`TokenProvider` requests client credentials from `/token`, retains only secret token text plus expiry in memory, refreshes when five minutes remain, and forcibly refreshes once after a 401. It never implements `Debug` for secret-bearing values and never returns the token to logging code.

- [ ] **Step 4: Implement sequential list and metadata collection**

`BlizzardClient` uses the fixed US endpoints and query. Retry only transport errors, timeout, 408, 429, 500, 502, 503 and 504; other HTTP failures return immediately. `Collector` requests pages one through the first response's `pageCount`, validates page/card totals and ID uniqueness, then fetches metadata. Reject page drift, missing pages, wrong top-level types and missing required metadata arrays as `PipelineError::ApiStructure`.

- [ ] **Step 5: Implement forward closure and class references**

Use a `BTreeSet<i64>` pending queue for deterministic ID order. Follow `childIds` and `bundledCardIds` recursively until closed. Do not follow `parentId` or `copyOfCardId`. Then fetch missing metadata class `cardId` and `heroPowerCardId`; exclude exact `alternateHeroCardIds` targets when closing relations from class-reference cards, but continue collecting non-skin gameplay targets such as upgraded hero powers. Preserve excluded skin relation IDs without requiring target rows. Enforce scope precedence `standard > class_reference > related` without replacing a list-page object with an individual response.

- [ ] **Step 6: Enforce two-locale structural parity**

`Collector::collect_all` returns only after both locales have identical Standard, related, class-reference and normalized taxonomy ID sets. Localized strings and image URLs are excluded from parity comparison. Add in-memory mutations for page drift, missing child target, request/response ID mismatch and locale ID mismatch.

- [ ] **Step 7: Verify GREEN and commit**

```powershell
cargo test -p card-data-pipeline --test collection --no-fail-fast
cargo clippy -p card-data-pipeline --all-targets -- -D warnings
git add -- crates/card-data-pipeline
git commit -m "feat(HCL-006): collect official card snapshots"
```

---

### Task 5: Normalize cards and write deterministic SQLite

**Files:**
- Modify: `crates/card-data-pipeline/Cargo.toml`
- Create: `crates/card-data-pipeline/src/normalize.rs`
- Create: `crates/card-data-pipeline/src/schema.sql`
- Create: `crates/card-data-pipeline/src/sqlite.rs`
- Modify: `crates/card-data-pipeline/src/lib.rs`
- Create: `crates/card-data-pipeline/tests/normalize_sqlite.rs`

**Interfaces:**
- Consumes: `CollectedLocale`, metadata, Task 2 normalized types, canonical Raw SHA-256, data version and injected generated time.
- Produces: `normalize_locale`, `SqliteBuildMetadata`, `SqliteWriter::write`, one complete locale DB, logical validation and deterministic insertion order.

- [ ] **Step 1: Write failing normalization assertions**

Assert scope precedence, empty-to-null behavior, no locale fallback, `text_markup` equality, exact `text_plain`, rune/sideboard all-or-none groups, `imageGold` omission, source-field mapping and relation order. Include this dangling relation assertion:

```rust
assert!(catalog.relations.iter().any(|relation|
    relation.kind == RelationKind::Parent &&
    relation.source_field == SourceField::ParentId &&
    relation.target_card_id == 9999
));
```

Run `cargo test -p card-data-pipeline --test normalize_sqlite` and confirm RED.

- [ ] **Step 2: Implement normalized mapping**

Add `rusqlite = "0.40.1"` with the `bundled` feature. Normalize known official fields only. Create taxonomy placeholders when referenced IDs are absent. Sort taxonomy/card rows by ID and join/relation rows by their declared primary keys. Treat missing official boolean flags as false. Reject partial rune/sideboard objects, duplicate official array IDs, negative constrained values and relation kind/source mismatches.

- [ ] **Step 3: Copy the approved DDL exactly into `schema.sql`**

Use the complete SQL block from section 10.1 of the approved spec, including 13 `STRICT` tables, five explicit indexes, lowercase-hex Raw hash check, two text columns, relation `source_field`, no target FK, deferred class hero FKs and grouped rune/sideboard checks.

- [ ] **Step 4: Write SQLite in one transaction**

`SqliteBuildMetadata` carries `schema_version`, data version, locale, generated time, canonical Raw uncompressed SHA-256 and the three scope counts. `SqliteWriter::write(path, catalog, metadata)` inserts the singleton metadata row in the same transaction as the normalized rows.

Before table creation set:

```sql
PRAGMA encoding = 'UTF-8';
PRAGMA page_size = 4096;
PRAGMA auto_vacuum = NONE;
PRAGMA journal_mode = DELETE;
PRAGMA foreign_keys = ON;
PRAGMA synchronous = FULL;
PRAGMA user_version = 1;
```

Insert metadata/taxonomy/cards/joins/relations in deterministic order, commit once, then require `PRAGMA foreign_key_check` to return zero rows and `PRAGMA integrity_check` to return exactly `ok`.

- [ ] **Step 5: Test logical structure and repeat bytes**

Assert table count 13, explicit index count 5, exact metadata counts, locale strings, relations, rune/sideboard values and absence of FTS/`name_choseong`. Write the same catalog twice with the bundled SQLite version and assert identical file SHA-256.

- [ ] **Step 6: Verify GREEN and commit**

```powershell
cargo test -p card-data-pipeline --test normalize_sqlite --no-fail-fast
cargo test -p card-data-pipeline --no-fail-fast
git diff --check
git add -- crates/card-data-pipeline
git commit -m "feat(HCL-006): normalize cards into SQLite"
```

---

### Task 6: Build deterministic compressed packages and manifest

**Files:**
- Modify: `crates/card-data-pipeline/Cargo.toml`
- Create: `crates/card-data-pipeline/src/package.rs`
- Modify: `crates/card-data-pipeline/src/lib.rs`
- Create: `crates/card-data-pipeline/tests/package.rs`

**Interfaces:**
- Consumes: canonical Raw bytes, locale SQLite files, version constants and injected clock.
- Produces: `PackageBuilder::build`, `validate_package_directory`, deterministic `.zst` assets, validated manifest and atomic final directory.

- [ ] **Step 1: Write failing deterministic package test**

With fixed fixture responses, clock `2026-08-05T00:00:00Z` and data version `36.0.3-build247416-r1`, run two builds under different temporary roots. Assert byte equality for both Raw files, both SQLite files, their four zstd assets and `manifest.json`.

- [ ] **Step 2: Implement hashes and zstd settings**

Add `zstd = "0.13.3"` with `zstdmt`, `sha2 = "0.11"`, `hex = "0.4"` and `tempfile = "3.27"`. Use streaming SHA-256. Configure one zstd frame per file with level 10, one worker, content size, frame checksum, no dictionary and the Cargo-locked zstd dependency. Record compressed and uncompressed bytes/hashes in `AssetDescriptor`.

- [ ] **Step 3: Build and validate manifest**

Create only four asset entries. Use camelCase serialization, exact locale count objects, parsed official patch/build/revision, common generated time and the Raw uncompressed hash in each SQLite metadata row. Serialize compact canonical JSON with one LF and run `Manifest::validate` before publication.

- [ ] **Step 4: Implement atomic staging and collision refusal**

Create a unique staging sibling under `output_root`; never create the final version path early. If the final path already exists, return CLI/config error without modifying it. On any error remove staging. After all hashes and self-validation succeed, atomically rename staging to `<output_root>/<data-version>`.

- [ ] **Step 5: Add fault injection tests**

An internal `PackageIo` trait has production filesystem operations and a test implementation that fails at: Raw write, SQLite write, compression, hash, manifest write and final rename. Every failure must leave neither staging nor final manifest/version directory. A successful output contains exactly the four assets and manifest.

- [ ] **Step 6: Verify GREEN and commit**

```powershell
cargo test -p card-data-pipeline --test package --no-fail-fast
cargo clippy -p card-data-pipeline --all-targets -- -D warnings
git add -- crates/card-data-pipeline
git commit -m "feat(HCL-006): package deterministic card assets"
```

---

### Task 7: Add the CLI, JSONL logging and exit-code contract

**Files:**
- Modify: `crates/card-data-pipeline/Cargo.toml`
- Create: `crates/card-data-pipeline/src/logging.rs`
- Modify: `crates/card-data-pipeline/src/main.rs`
- Modify: `crates/card-data-pipeline/src/lib.rs`
- Create: `crates/card-data-pipeline/tests/cli.rs`

**Interfaces:**
- Consumes: all pipeline services and stable `PipelineError` variants.
- Produces: `BuildRequest`, `BuildResult`, `run_build`, `EventSink`, test-only `VecEventSink`, `card-data-pipeline build --data-version <value> --output-root <path>`, stderr JSONL, empty stdout and exit codes 0/2/3/4/5/6/7.

- [ ] **Step 1: Write failing command tests**

Add `clap = "4.6"` with derive and dev dependencies `assert_cmd = "2.2"` and `predicates = "3.1"`. Use `assert_cmd` to verify missing credentials exits 2, invalid version exits 2, existing output exits 2, OAuth failure exits 3, retry exhaustion exits 4, API structure failure exits 5, normalization failure exits 6 and package I/O failure exits 7. Parse every non-empty stderr line as JSON and assert stdout is empty.

- [ ] **Step 2: Implement clap command validation**

Expose exactly one subcommand:

```rust
#[derive(clap::Subcommand)]
enum Command {
    Build {
        #[arg(long)]
        data_version: String,
        #[arg(long)]
        output_root: PathBuf,
    },
}
```

Read only `BLIZZARD_CLIENT_ID` and `BLIZZARD_CLIENT_SECRET` from process environment. Do not read or print `.env.card-data.local`; local instructions load it before starting the process.

The library entry point used by both the CLI and live smoke is:

```rust
pub struct BuildRequest {
    pub data_version: DataVersion,
    pub output_root: PathBuf,
    pub credentials: Credentials,
}

pub struct BuildResult {
    pub version_directory: PathBuf,
    pub manifest: Manifest,
}

pub async fn run_build(
    request: BuildRequest,
    events: &mut dyn EventSink,
) -> Result<BuildResult, PipelineError>;
```

- [ ] **Step 3: Implement JSONL events and redaction**

Every event contains `schema_version`, RFC 3339 `timestamp`, `level`, `stage`, `event`; optional fields are only locale, attempt, status code, counts, error code and safe message. Emit stage start/completion, retry, locale summary and final result. Do not emit successful-request events or URLs/queries containing credentials.

`EventSink::emit(&mut self, event: Event) -> io::Result<()>` is implemented by `JsonlEventSink<W: Write>` for production stderr and `VecEventSink` for assertions. Neither sink accepts request or credential objects.

- [ ] **Step 4: Map typed errors to exact exits**

Use this direct mapping:

```rust
match error {
    PipelineError::Cli(_) | PipelineError::Config(_) => 2,
    PipelineError::Auth(_) => 3,
    PipelineError::Network(_) => 4,
    PipelineError::ApiStructure(_) => 5,
    PipelineError::Normalize(_) | PipelineError::Sqlite(_) => 6,
    PipelineError::Package(_) | PipelineError::Io(_) => 7,
}
```

Before exiting, log one safe final failure event. Error messages must not wrap raw reqwest request/debug values that could include an Authorization header.

- [ ] **Step 5: Verify redaction and commit**

```powershell
cargo test -p card-data-pipeline --test cli --no-fail-fast
cargo test -p card-data-pipeline --no-fail-fast
git diff --check
git add -- crates/card-data-pipeline
git commit -m "feat(HCL-006): expose card pipeline CLI"
```

---

### Task 8: Add end-to-end verification, ignored live smoke and usage docs

**Files:**
- Create: `crates/card-data-pipeline/tests/live_smoke.rs`
- Modify: `README.md`
- Create: `.env.example`
- Modify: `package.json` if the full gate needs a final Rust command adjustment

**Interfaces:**
- Consumes: the production build path from Tasks 1-7.
- Produces: offline end-to-end proof, explicit credentialed smoke command, secret-safe setup docs and final HCL-006 verification evidence.

- [ ] **Step 1: Add an offline full-pipeline integration test**

Serve Task 3 fixtures through wiremock, invoke the same library entry point as the CLI, and assert the exact five-file output, self-validating manifest, two valid SQLite databases and no residual staging directory. The test must not use a smoke-only collector or reduced production branch.

- [ ] **Step 2: Add the ignored live smoke**

Use:

```rust
#[tokio::test]
#[ignore = "requires Blizzard credentials and network"]
async fn builds_current_standard_package_for_both_locales() {
    let credentials = Credentials::from_env().expect("Blizzard credentials");
    let output = tempfile::tempdir().expect("temporary output root");
    let request = BuildRequest {
        data_version: DataVersion::parse("36.0.3-build247416-r1").unwrap(),
        output_root: output.path().to_path_buf(),
        credentials,
    };
    let mut events = VecEventSink::default();
    let result = run_build(request, &mut events).await.expect("live package");

    assert_eq!(result.manifest.schema_version, SCHEMA_VERSION);
    assert_eq!(
        result.manifest.supported_locales,
        vec!["ko_KR".to_owned(), "en_US".to_owned()],
    );
    validate_package_directory(&result.version_directory, &result.manifest).unwrap();
    drop(result);
    output.close().expect("remove live-smoke output");
}
```

The implementation must never print credentials/token and must delete temporary data after handles are closed on Windows.

- [ ] **Step 3: Document local execution without secrets**

Add `.env.example` containing variable names with empty values only:

```dotenv
BLIZZARD_CLIENT_ID=
BLIZZARD_CLIENT_SECRET=
```

README commands:

```powershell
cargo test --workspace
cargo test -p card-data-pipeline --test live_smoke -- --ignored
cargo run -p card-data-pipeline -- build --data-version 36.0.3-build247416-r1 --output-root .\output
```

Explain that PowerShell/local tooling loads `.env.card-data.local` into process environment and that the pipeline itself never copies the file.

- [ ] **Step 4: Run focused and full verification**

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
npm run check
git diff --check
```

Expected: every offline Rust/TypeScript/tracking test passes, frontend build succeeds, Tauri and both data crates compile, and the worktree contains no generated output.

- [ ] **Step 5: Run the credentialed live smoke explicitly**

Load the local ignored credential file into the process environment without printing it, then run:

```powershell
cargo test -p card-data-pipeline --test live_smoke -- --ignored
```

Expected: current `ko_KR` and `en_US` Standard packages are built and self-validated in an OS temporary directory, then removed.

- [ ] **Step 6: Commit the verification slice**

```powershell
git add -- .env.example README.md package.json crates/card-data-pipeline/tests/live_smoke.rs
git commit -m "test(HCL-006): verify official card pipeline"
```

---

## Plan Self-Review Checklist

- Every approved spec section maps to Tasks 1-8: responsibility boundary, CLI/secrets, OAuth, API scope, retry, Raw, normalization, SQLite, output, zstd, manifest, JSONL, exit codes and tests.
- Related/class-reference closure and intentional dangling parent/copy relations are both covered.
- `text_markup`, `text_plain`, `source_field`, schema constants and hash validation have explicit RED/GREEN tests.
- No task introduces images, R2, signatures, diffs, updater, IPC, frontend search, FTS or semantic inference.
- Interface names used by later tasks are introduced by earlier tasks.
- The plan contains no implementation placeholder or unspecified “add tests” step.
- Main-only tracking files are not touched by implementation tasks.

## Merge Preparation After Implementation

After Task 8, do not merge immediately. Update the feature branch with current `main`, rerun the full verification, run `/va HCL-006`, then run `npm run merge:check -- HCL-006`. Only a clean VA result and passing merge gate permit the documented squash-merge workflow on `main`.
