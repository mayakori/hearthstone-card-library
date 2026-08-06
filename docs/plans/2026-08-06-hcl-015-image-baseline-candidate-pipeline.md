# HCL-015 카드 이미지 기준팩 후보 파이프라인 구현 계획

## 목표와 완료 증거

검증된 HCL-006 패키지에서 두 locale의 normal/crop 이미지 기준팩 candidate를 만들고, 별도 수동 GitHub Actions가 R2에 pack/map을 올린 뒤 다시 내려받아 Rust verifier를 통과시키고 receipt를 마지막에 게시한다.

완료 증거는 fixture 기반 RED→GREEN 테스트, 결정성·변조 거부 테스트, 전체 `npm run check`, `/va HCL-015`, merge gate와 credentialed R2 full smoke다.

## 구현 경계

- 수정 가능: `crates/card-data-pipeline/`, `.github/workflows/`, `scripts/`, `tests/`, HCL-015 spec/plan과 README
- 수정하지 않음: `src/`, `src-tauri/`, HCL-006 schema, HCL-014 workflow, 앱 updater와 public pointer
- feature worktree에서 main 전용 tracking 파일을 수정하지 않음
- 모든 production behavior는 실패하는 테스트를 먼저 확인한 뒤 구현

## 단계 1. 검증 패키지에서 이미지 요청 추출

### RED

- 새 `tests/image_baseline.rs`가 유효한 두-locale package fixture에서 모든 카드 ID의 normal/crop 요청을 안정된 순서로 기대한다.
- URL `NULL`은 absent로 남고, locale 행 수·ID parity 불일치와 gold/input-out-of-scope 주입은 거부되는 계약을 고정한다.

### GREEN

- `image/source.rs`가 HCL-006 package 검증, SQLite 제한 해제와 요청 추출을 담당한다.
- `image/model.rs`에 locale/variant/request와 안정된 오류 context를 둔다.
- 기존 package 검증 로직을 복제하지 않고 공개 가능한 최소 validator/decompress helper만 재사용한다.

### 확인

- `cargo test -p card-data-pipeline --test image_baseline source_ --all-features`

## 단계 2. bounded 이미지 downloader와 미디어 검증

### RED

- Wiremock fixture로 PNG/JPEG/WebP 성공, 유효한 `application/octet-stream` 감지, absent 무요청, 4xx unavailable 보존, 429/5xx retry, `Retry-After`, timeout, HTTPS/redirect, 10 MiB cap과 header/signature 불일치를 검증한다.
- expected bytes/hash/media type은 코드 helper가 아니라 hand-checked literal fixture로 고정한다.

### GREEN

- `image/download.rs`에 credential-free reqwest client, concurrency 4, 30초 timeout과 initial+3 retry를 구현한다.
- 결과는 요청 key와 원본 bytes, SHA-256, media type/extension만 반환하고 완료 순서는 외부 계약에 노출하지 않는다.
- retry log는 기존 안전한 event sink 계약을 확장하거나 image 전용 scalar summary로 제한한다.

### 확인

- `cargo test -p card-data-pipeline --test image_baseline download_ --all-features`

## 단계 3. 중복 제거, map, 결정적 tar.zst와 receipt

### RED

- 다른 URL/같은 bytes가 한 canonical member만 만들고 모든 map entry가 owner pack을 가리키는 테스트를 쓴다.
- 다른 루트에서 두 번 만든 map/pack/receipt exact bytes가 같아야 한다.
- 작은 test pack cap에서 hash 연속 shard가 결정적으로 나뉘어야 한다.
- tar member 변조, path traversal, duplicate hash와 map dangling reference를 verifier가 거부해야 한다.

### GREEN

- `image/pack.rs`가 global hash dedupe, owner 선택, deterministic tar header, zstd level 3과 shard를 구현한다.
- `image/map.rs`가 두 canonical JSON zstd map을 만든다.
- `image/receipt.rs`가 입력 package identity, aggregate count와 모든 object digest를 고정한다.
- `image/verify.rs`가 local 또는 R2 download root의 map/pack/member를 receipt 기준으로 전부 검증한다.

### 확인

- `cargo test -p card-data-pipeline --test image_baseline pack_ --all-features`
- `cargo test -p card-data-pipeline --test image_baseline verify_ --all-features`

## 단계 4. production CLI orchestration

### RED

- CLI integration test가 잘못된 package, 기존 output, 잘못된 run identity와 변조 candidate에 안정된 JSONL/exit code를 기대한다.
- 유효한 local HTTP fixture는 `image-baseline-build` 후 `image-baseline-verify`까지 production orchestration을 통과해야 한다.

### GREEN

- `image-baseline-build --package-root --output-root --run-id --run-attempt`
- `image-baseline-verify --candidate-root`
- candidate build는 staging directory에서 완성·검증 후 run-unique final directory로 atomic publish한다.
- CLI는 Blizzard/R2 secret을 요구하거나 출력하지 않는다.

### 확인

- `cargo test -p card-data-pipeline --test cli --all-features`
- `cargo test -p card-data-pipeline --test image_baseline --all-features`

## 단계 5. R2 helper와 manual workflow

### RED

- Node contract test가 실제 fixture receipt를 실행해 pack/map upload entries, download layout와 receipt-last entries를 검증한다.
- 변조된 download bytes, receipt 조기 포함, Raw/SQLite/pointer object를 거부한다.
- workflow 실행 계약은 manual-only, `contents: read`, 90분, production binary 명시, pack/map remote verify 후 receipt upload 순서를 검증한다.

### GREEN

- `scripts/card-data-image-r2-candidate.mjs`가 canonical receipt를 읽어 안전한 TSV/JSON 출력과 download-root object 검증 경계를 제공한다.
- `.github/workflows/card-data-image-r2-candidate.yml`이 HCL-006 build, image build, metadata artifact, R2 pack/map upload, remote download, Rust verify, receipt-last upload/get을 연결한다.
- `package.json`의 workflow test는 Raw와 image Node tests를 모두 독립 Node runner로 실행한다.
- README에 수동 실행, 저장 경로와 비발행 경계를 기록한다.

### 확인

- `npm run workflow:test`
- YAML parse와 금지 object contract

## 단계 6. 통합 검증과 live smoke

1. `cargo fmt --all -- --check`
2. `npm run check`
3. 현재 main 반영 후 같은 검증 재실행
4. `/va HCL-015`
5. `npm run merge:check -- HCL-015`
6. 사용자 push 승인 후 main 병합·push
7. `36.0.3-build247416-r1` 또는 실행 시점의 승인 data version으로 manual workflow 실행
8. 성공 run의 pack/map/receipt 개수, unique image 수, bytes와 R2 prefix를 tracking 정본에 기록

## 중단 조건

- 공식 CDN이 승인 media type 이외 형식을 실제로 반환함
- 현재 전체 baseline이 90분 job 한도를 안정적으로 초과함
- 한 pack shard 또는 한 이미지가 승인 한도를 넘음
- HCL-006 package에서 locale별 카드 ID/URL 계약이 설계와 다름
- R2 remote verification이 동일 원인으로 반복 실패함

이 경우 구현을 임의 완화하지 않고 실제 증거와 영향 범위를 제시해 설계를 다시 승인받는다.
