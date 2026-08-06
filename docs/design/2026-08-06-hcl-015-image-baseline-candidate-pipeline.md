# HCL-015 카드 이미지 기준팩 후보 파이프라인 설계

## 1. 목표

HCL-006이 만든 검증된 현재 카드 패키지에서 `ko_KR`, `en_US`의 `normal`, `crop` 이미지 URL을 추출하고, 공식 이미지 바이트를 검증·해시·중복 제거해 결정적 기준팩과 카드별 이미지 맵을 만든다. 별도의 수동 GitHub Actions가 결과를 실행별 Cloudflare R2 candidate 경로에 올리고 다시 내려받아 검증한다.

이 작업은 이미지 정본 발행 전의 기준 candidate를 만드는 단계다. 앱이 읽는 공개 pointer나 안정 릴리스는 만들지 않는다.

## 2. 선행 정본과 우선순위

이 설계는 다음 문서의 확정안을 좁은 구현 단위로 구체화한다.

- `docs/superpowers/specs/2026-07-25-hcl-006-official-card-data-pipeline-design.md`
- `docs/design/card-data-architecture-decisions.md`
- `docs/design/card-data-r2-release-design.md`
- `docs/design/2026-08-06-hcl-014-r2-raw-candidate-workflow.md`

충돌 시 HCL-006의 실제 패키지 계약을 입력 경계로 사용하고, R2 release 설계의 이미지 정본 원칙을 유지한다. HCL-015는 release 설계를 대체하지 않으며 candidate 단계만 추가한다.

## 3. 범위

### 포함

- HCL-006 패키지 전체 검증 후 locale별 정규화 SQLite에서 이미지 요청 추출
- `ko_KR`, `en_US`
- `normal`, `crop`
- 원본 이미지 바이트 다운로드와 미디어 검증
- 실제 바이트 SHA-256 기반 전역 중복 제거
- 카드별 이미지 맵 두 개
- 결정적 baseline `tar.zst` 팩과 480 MiB 분할
- 실행별 R2 candidate 업로드, 재다운로드와 원격 바이트 검증
- 검증 완료 receipt의 마지막 업로드
- fixture, 패키징, downloader, workflow 계약 테스트

### 제외

- `gold` 이미지
- HCL-006 입력에 없는 영웅 스킨과 비게임 장식 이미지
- 이전 버전과 비교한 delta 팩
- 장기 전역 이미지 인덱스
- 최신 설치용 bootstrap 팩
- 정규화 SQLite 스키마 변경과 이미지 해시 열 추가
- `stable.json`, `versions.json`, `current` 계열 pointer
- 전자서명과 공개 배포
- Tauri 다운로드, 압축 해제와 로컬 이미지 캐시
- 예약 실행과 자동 패치 감지

## 4. 실제 입력 규모

2026-08-06 실환경 HCL-006 패키지 `36.0.3-build247416-r1`은 locale마다 다음 범위를 가진다.

| scope | 카드 수 |
|---|---:|
| Standard | 1,559 |
| related | 64 |
| class reference | 22 |
| 합계 | 1,645 |

상한 후보는 `1,645 × 2 locale × 2 variant = 6,580` 요청이다. 실제 요청 수는 URL 누락과 동일 URL 재사용으로 줄고, 팩 member 수는 바이트 SHA-256 중복 제거로 더 줄 수 있다. 구현은 이 숫자를 상수로 고정하지 않으며 입력 manifest와 SQLite의 완전성을 기준으로 삼는다.

2026-08-06 교정 후 같은 패키지의 local full smoke 결과는 다음과 같다.

| 결과 | 개수 |
|---|---:|
| 전체 슬롯 | 6,580 |
| 입력 URL 없음 (`null`) | 1,180 |
| 검증 성공 이미지 | 5,398 |
| 공식 4xx (`unavailable`) | 2 |
| SHA-256 중복 제거 후 고유 이미지 | 4,314 |

성공 이미지 중 5,328건은 공식 CDN이 `application/octet-stream`으로 표시했지만 실제 PNG/JPEG signature가 유효했다. `멀록 정찰병` ID `69571`의 normal 이미지 URL은 두 locale 모두 HTTP 403이어서 각각 `unavailable`로 보존됐다. 특정 카드 제외나 대체 이미지는 적용하지 않았다.

## 5. 입력 계약

이미지 파이프라인은 임의 JSON이나 외부 URL 목록을 직접 입력받지 않는다.

1. HCL-006 `manifest.json`과 네 asset을 `validate_package_directory`와 동일한 계약으로 검증한다.
2. locale별 `normalized/<locale>.sqlite.zst`를 제한된 임시 디렉터리에 해제한다.
3. `cards`의 `id`, `scope`, `image_url`, `crop_image_url`만 읽는다.
4. manifest의 locale별 `total`과 SQLite 카드 행 수가 같아야 한다.
5. 두 locale의 카드 ID 집합이 같아야 한다.

HCL-006에서 제거한 영웅 스킨을 이미지 단계에서 다시 발견하거나 별도 API로 보충하지 않는다. 이미지 범위는 검증된 패키지의 카드 행 범위와 정확히 같다.

## 6. 요청 모델

내부 요청은 다음 필드로 고정한다.

```text
ImageRequest
├─ card_id: u64
├─ locale: ko_KR | en_US
├─ variant: normal | crop
└─ source_url: Option<HTTPS URL>
```

정렬 순서는 locale `ko_KR`, `en_US`, 카드 ID 오름차순, variant `normal`, `crop`이다. 이 순서는 다운로드 결과의 완료 순서와 무관하게 map과 팩 소유권을 결정한다.

- URL이 `NULL`이면 정상적인 `absent` 상태다. 네트워크 요청을 만들지 않는다.
- URL이 있고 429 이외 4xx가 반환되면 출처 URL, `http_status` 사유와 상태 코드를 `unavailable`로 보존한다.
- 그 외 URL은 성공한 이미지 바이트가 반드시 있어야 한다.
- 다른 locale, variant나 이전 버전 이미지로 대체하지 않는다.
- `imageGold`는 Raw에 있더라도 읽지 않는다.

## 7. 다운로드와 미디어 검증

### 네트워크

- 전역 동시 요청 수는 4다.
- 요청당 전체 타임아웃은 30초다.
- 초기 요청 뒤 최대 3회 재시도한다.
- 재시도 대상은 연결·읽기 오류, HTTP 429와 5xx다.
- `Retry-After`가 유효하면 우선하고, 없으면 bounded exponential backoff를 사용한다.
- 4xx 중 429 이외 상태는 재시도하지 않고 `unavailable`로 보존한다. 미디어 검증 실패는 재시도하지 않고 실행을 실패시킨다.
- 최초 URL과 redirect 목적지는 HTTPS만 허용하고 redirect는 최대 3회다.
- Blizzard API 자격증명과 R2 자격증명을 이미지 CDN 요청에 전달하지 않는다.

### 바이트 한도와 형식

- 응답 한 장의 최대 크기는 10 MiB다.
- `Content-Length`가 한도를 넘으면 body를 읽기 전에 거부한다.
- 길이를 알 수 없어도 streaming read 중 10 MiB를 넘으면 중단한다.
- 허용되는 실제 media type은 `image/png`, `image/jpeg`, `image/webp`다.
- 응답 header가 위 media type이면 파일 signature와 반드시 일치해야 한다.
- 공식 CDN이 사용하는 `application/octet-stream`은 PNG/JPEG/WebP signature가 유효한 경우에만 허용하고, 저장 media type과 확장자는 실제 bytes에서 결정한다.
- 그 밖의 header, signature와 실제 bytes가 불일치하면 전체 실행을 실패시킨다.
- 원본 바이트를 재인코딩하거나 metadata를 제거하지 않는다.

URL이 존재하는데 429 이외 4xx가 나오면 해당 asset만 `unavailable`로 남는다. 제한 초과, 빈 body, 허용되지 않은 형식 또는 손상된 signature가 나오면 해당 candidate 전체가 실패한다. 특정 카드 ID를 하드코딩해 제외하거나 다른 이미지로 대체하지 않는다.

## 8. 해시와 중복 제거

각 성공 응답에 대해 원본 bytes의 SHA-256을 계산한다. 같은 hash는 URL, 카드, locale과 variant가 달라도 하나의 canonical member만 가진다.

동일 hash를 참조하는 요청 중 6장의 정렬 순서에서 가장 앞선 요청이 canonical owner가 된다. owner의 locale과 variant가 그 member를 담을 팩 그룹을 정한다. 나머지 map entry는 owner의 pack/member를 참조한다.

같은 hash에서 감지 media type이나 extension이 다르면 내부 계약 위반으로 실패한다. canonical extension은 다음과 같다.

| media type | extension |
|---|---|
| `image/png` | `png` |
| `image/jpeg` | `jpg` |
| `image/webp` | `webp` |

member 이름은 `<sha256>.<extension>`이다. 카드명, slug와 URL 문자열은 archive 경로에 사용하지 않는다.

## 9. locale 이미지 맵

locale마다 canonical JSON을 만든 뒤 zstd로 압축한다.

```json
{
  "schemaVersion": 1,
  "dataVersion": "36.0.3-build247416-r1",
  "locale": "ko_KR",
  "cards": [
    {
      "cardId": 1001,
      "normal": {
        "state": "available",
        "sourceUrl": "https://example.invalid/card.png",
        "sha256": "<64 lowercase hex>",
        "bytes": 123456,
        "mediaType": "image/png",
        "pack": "packs/ko_KR-normal-000.tar.zst",
        "member": "<sha256>.png"
      },
      "crop": {
        "state": "unavailable",
        "sourceUrl": "https://example.invalid/card-crop.png",
        "reason": "http_status",
        "statusCode": 403
      }
    }
  ]
}
```

- `cards`는 모든 입력 카드 ID를 정확히 한 번 포함하고 ID 오름차순이다.
- `normal`과 `crop`은 `available` asset 객체, `unavailable` 상태 객체 또는 `null`이다.
- `null`은 입력 URL이 없었다는 뜻이며 다운로드 실패를 뜻하지 않는다.
- `unavailable`은 입력 URL은 있었지만 공식 서버가 429 이외 4xx로 자산을 제공하지 않았다는 뜻이다.
- cross-locale/variant dedupe 때문에 `pack`이 다른 locale 또는 variant 이름일 수 있다.
- source URL은 감사와 이후 diff 후보 계산을 위해 보존한다.
- map은 HCL-006 SQLite를 수정하지 않는다. 정식 release schema에 hash를 병합하는 일은 후속 작업이다.

경로는 `maps/ko_KR.json.zst`, `maps/en_US.json.zst`다.

## 10. 결정적 이미지팩

### tar member

- 각 canonical hash는 전체 baseline candidate에서 정확히 한 pack에만 들어간다.
- 그룹 안에서 member는 SHA-256 오름차순이다.
- member 경로는 파일명 한 segment만 허용한다.
- 중복 member, 절대경로, `..`, symlink, hardlink와 device entry를 만들지 않는다.
- tar header의 mtime, uid, gid, uname, gname과 mode를 고정한다.
- 실제 image bytes만 member body로 기록한다.

### zstd

- 저장소 lockfile의 zstd 구현을 단일 thread, level 3으로 고정한다. 이미 압축된 이미지의 컨테이너화가 목적이므로 높은 압축 level을 사용하지 않는다.
- content size와 frame checksum을 포함한다.
- 입력 순서와 실행 경로가 달라도 같은 bytes를 만든다.
- 생성 직후 해제해 member 수, 이름, 크기와 SHA-256을 다시 검증한다.

### 분할

팩 이름은 `<owner-locale>-<owner-variant>-<shard:03>.tar.zst`다. pack 결과가 480 MiB를 넘지 않게 hash 순서의 연속 구간으로 결정적으로 분할한다. 한 이미지 자체가 pack 한도를 넘으면 분할하지 않고 실패한다.

## 11. candidate receipt

로컬 receipt는 다음을 고정한다.

- schema version, data version, GitHub run ID와 attempt
- 입력 manifest SHA-256과 locale별 normalized asset SHA-256
- 요청, absent, unavailable, 성공 응답과 unique image 개수
- media type별 개수와 총 원본 byte 수
- pack과 map 각각의 R2 상대경로, bytes와 SHA-256
- pack별 member 개수와 해제 후 총 bytes
- candidate prefix

비밀값, bearer token, R2 endpoint 전체 문자열과 임시 로컬 절대경로는 포함하지 않는다.

## 12. R2 candidate 발행

별도 수동 workflow만 사용한다.

```text
candidates/images/<data-version>/runs/<github-run-id>-<run-attempt>/
├─ packs/
│  ├─ ko_KR-normal-000.tar.zst
│  ├─ ko_KR-crop-000.tar.zst
│  ├─ en_US-normal-000.tar.zst
│  └─ en_US-crop-000.tar.zst
├─ maps/
│  ├─ ko_KR.json.zst
│  └─ en_US.json.zst
└─ receipt.json
```

실제 pack 소유권과 shard 수에 따라 일부 예시 pack이 없거나 `001` 이상이 생길 수 있다.

발행 순서는 다음과 같다.

1. 전체 입력, 이미지, map과 pack을 runner 임시 경로에서 완성·검증한다.
2. pack을 R2에 업로드한다.
3. map을 R2에 업로드한다.
4. pack과 map을 새 임시 경로로 전부 다시 내려받는다.
5. 객체 bytes/SHA-256과 archive의 모든 member SHA-256을 검증한다.
6. 검증된 canonical receipt JSON을 마지막에 업로드한다.
7. receipt를 다시 내려받아 exact bytes와 SHA-256을 확인한다.

유효한 `receipt.json`과 receipt가 고정한 모든 원격 hash의 일치는 candidate 완료 표식이다. receipt가 없거나 canonical JSON·hash 검증에 실패한 prefix는 불완전한 실패 실행이며 소비하거나 승격하지 않는다.

candidate 객체는 `Cache-Control: no-store`를 사용한다. lifecycle 정리는 `candidates/images/` prefix에 별도 정책으로 적용하며 workflow가 기존 candidate를 삭제하지 않는다.

## 13. GitHub Actions 경계

- trigger는 `workflow_dispatch` 하나다.
- 입력은 HCL-006 형식의 `data_version` 하나다.
- HCL-006 production CLI로 현재 package를 새로 수집·검증한다.
- 이미지 단계는 검증된 package root만 입력받는다.
- job timeout은 최초 baseline을 고려해 90분으로 둔다.
- workflow permission은 `contents: read`만 사용한다.
- 기존 HCL-014 workflow를 변경하거나 Raw candidate를 다시 발행하지 않는다.
- Secrets와 Variables는 HCL-014와 같은 이름을 재사용한다.
- GitHub Artifact에는 receipt, 두 map과 안전한 구조화 로그만 7일 보존한다. 대형 image pack은 R2 candidate가 정본이므로 Artifact에 중복 보존하지 않는다.
- 실패 job을 rerun하면 새 `run-attempt` prefix에서 처음부터 실행한다. 이전 실패 prefix를 재개하거나 덮어쓰지 않는다.

## 14. 로그 계약

JSONL 이벤트는 안전한 scalar만 기록한다.

- 단계와 attempt
- 전체/완료/absent/unavailable/unique 개수
- retry attempt와 HTTP status
- 누적 bytes
- pack/map 개수와 검증 결과
- 실패한 card ID, locale, variant와 안정된 오류 종류

Blizzard/R2 자격증명과 HTTP authorization header는 어떤 오류에도 포함하지 않는다. source URL은 map에는 보존하지만 기본 로그에는 출력하지 않는다.

## 15. 실패와 재실행

- receipt 업로드 전 실패: candidate 미완료, 공개 상태 변화 없음
- 일부 pack/map 업로드 뒤 실패: receipt 없음, lifecycle 정리 대상
- receipt 업로드 또는 재다운로드 실패: 전체 workflow 실패
- GitHub rerun: 처음부터 다시 수집·다운로드·패키징하고 새 attempt prefix 사용
- 기존 성공 candidate: 덮어쓰거나 삭제하지 않음

HCL-015는 checkpoint/resume을 만들지 않는다. bounded retry와 candidate 격리로 MVP 실패 계약을 단순하게 유지한다.

## 16. 구현 경계

이미지 처리 코드는 Rust backend/Tauri runtime이 아니라 기존 `card-data-pipeline` crate 안의 독립 모듈로 둔다. 네트워크, 미디어 검증, mapping, pack 생성과 receipt 검증을 분리하되 이 슬라이스를 위해 새 workspace crate나 범용 plugin 계층을 만들지 않는다.

예상 책임은 다음과 같다.

```text
image source reader
→ verified HCL-006 package에서 요청 생성

image downloader
→ bounded concurrency, retry, byte/media validation

image packager
→ SHA-256 dedupe, map, deterministic tar.zst

image candidate verifier
→ local/remote object와 archive member 검증

GitHub workflow/helper
→ R2 전송과 receipt-last 순서
```

SolidJS, `src-tauri` IPC와 앱 로컬 캐시는 변경하지 않는다.

## 17. 테스트 계약

### fixture와 단위 테스트

- 두 locale, normal/crop, URL 없음과 cross-locale 중복
- PNG, JPEG, WebP signature와 header 일치 및 유효한 `application/octet-stream` 감지
- 4xx unavailable 보존, 429/5xx retry, timeout, 잘못된 media type/signature, 빈 body와 10 MiB 초과
- `Retry-After` 우선과 최대 3회 retry
- redirect 제한과 HTTPS 강제
- 같은 URL/다른 bytes, 다른 URL/같은 bytes
- gold와 입력 외 카드가 요청에 들어가지 않음

### 패키징 테스트

- 같은 입력을 다른 루트에서 두 번 만들 때 map, pack과 receipt bytes 동일
- 같은 image hash가 두 pack에 들어가지 않음
- 모든 non-null map entry가 정확한 pack/member를 가리킴
- 모든 입력 카드가 map에 존재하고 absent만 null이며 4xx는 unavailable 객체
- 낮춘 테스트 한도에서 결정적 multi-shard 분할
- tar path 탈출, duplicate member와 변조 거부
- pack/map/receipt 원격 다운로드 bytes 변조 거부

### workflow 계약

- manual-only, `contents: read`, 90분 timeout
- production package 입력과 승인된 네 Secrets·두 Variables 사용
- R2에 packs/maps/receipt 외 객체를 만들지 않음
- receipt-last와 원격 재다운로드 검증
- `stable`, `versions`, `current`, normalized DB와 Raw 업로드 금지

### 실환경 smoke

- 현재 data version으로 full workflow 성공
- R2 candidate receipt 존재
- 두 locale map의 카드 수가 manifest total과 일치
- gold asset 0개
- 원격 pack/map/receipt 검증 성공
- 실제 object prefix와 안전한 aggregate count/bytes를 tracking 문서에 기록

## 18. 완료 조건

- HCL-006 검증 패키지만 이미지 범위의 입력 정본으로 사용한다.
- 두 locale의 normal/crop을 원본 bytes로 검증하고 누락과 실패를 구분한다.
- SHA-256 중복 제거 후 각 unique image가 정확히 한 baseline pack에 존재한다.
- map, pack과 receipt가 결정적이며 모든 참조가 완전하다.
- candidate prefix에서 원격 재다운로드 검증이 통과한 뒤 receipt가 마지막에 존재한다.
- 실패 실행은 receipt가 없고 공개 pointer나 기존 성공 candidate를 바꾸지 않는다.
- 전체 저장소 검증, `/va HCL-015`, `npm run merge:check -- HCL-015`와 credentialed R2 smoke가 통과한다.

## 19. 후속 작업

HCL-015 승인·구현 후 다음 항목은 별도 작업으로 다룬다.

1. 이전 이미지 맵과 비교한 canonical delta 및 장기 global image index
2. 최신 locale 설치용 bootstrap 팩과 7일 교체 보존
3. 정규화 SQLite·version manifest에 이미지 hash 연결
4. 서명과 pointer-last production 승격
5. Tauri 이미지팩 설치, 로컬 content-addressed cache와 placeholder UX
