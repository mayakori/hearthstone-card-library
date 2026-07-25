# 카드 데이터 R2 배포·버전 설계

Status: Approved design

Approved: 2026-07-25 KST

Scope: 공식 카드 데이터 수집 결과, 패치 이력, 카드 이미지의 서버리스 배포와 Tauri 로컬 갱신

## 1. 목적

공식 Hearthstone Game Data API에서 수집한 카드 데이터를 버전별로 보존하고, Cloudflare R2를 통해 데스크톱 앱에 배포한다.

이 설계가 만족해야 하는 핵심 조건은 다음과 같다.

- 앱 바이너리 업데이트와 카드 데이터 업데이트를 분리한다.
- 사용자는 Blizzard API 키나 R2 업로드 자격증명을 발급받지 않는다.
- 각 공식 패치 시점의 카드 데이터를 즉시 열람할 수 있다.
- 이미지가 변경된 버전에서는 당시 이미지까지 정확히 표시한다.
- 동일한 이미지 바이트를 버전마다 정본으로 중복 저장하지 않는다.
- 신규 사용자는 최신 이미지를 여러 과거 패치팩 없이 편리하게 설치할 수 있다.
- 다운로드·검증·압축 해제 중 실패하면 기존 로컬 데이터를 유지한다.
- 초기 지원 언어는 `ko_KR`, `en_US`이며 이후 앱 지원 언어와 함께 확장한다.

## 2. 기존 결정과의 관계

이 문서는 `docs/handoff-card-data-release.md`에서 GitHub Release Assets를 카드 데이터 배포 원본으로 삼았던 부분을 대체한다.

다음 결정은 유지한다.

- 카드 메타데이터의 정본은 Blizzard 공식 Hearthstone Game Data API다.
- 배포자의 Blizzard API 자격증명은 신뢰된 CI에서만 사용한다.
- 원본 응답과 앱용 정규화 모델을 구분한다.
- 앱 업데이트와 카드 데이터 업데이트는 서로 다른 갱신기가 담당한다.
- 로컬 이미지 캐시는 다시 생성 가능한 장치별 캐시다.
- 장기 스냅샷 보관과 Blizzard API 데이터 TTL 조건의 충돌은 인지하고 수용한 위험으로 기록한다.

변경된 배포 경계는 다음과 같다.

| 대상 | 배포 원본 |
|---|---|
| Tauri 앱 설치 파일·업데이터 메타데이터 | GitHub Releases |
| 카드 원본·정제 데이터 | Cloudflare R2 |
| 카드 normal·crop 이미지 | Cloudflare R2 |
| CI 중간 검증 산출물 | GitHub Actions Artifact |

## 3. 전체 구조

```text
공식 패치노트                    Blizzard Game Data API
      │                                  │
      └────────────┬─────────────────────┘
                   ▼
             GitHub Actions
      ├─ 패치 이벤트 탐지
      ├─ API 원본 수집
      ├─ 정규화·스냅샷·diff 생성
      ├─ 변경 이미지 선별·해시 계산
      ├─ 정본팩·최신 설치팩 생성
      ├─ 스키마·SHA-256 검증
      └─ 제어 문서 전자서명
                   │
                   ▼
              Cloudflare R2
      ├─ stable / versions
      ├─ 버전별 manifest
      ├─ raw / normalized / diff
      ├─ 이미지 정본 기준팩·변경팩
      └─ 최신 전체 이미지 설치팩
                   │
                   ▼
            Tauri CardDataUpdater
      ├─ 서명·해시 검증
      ├─ 임시 경로 다운로드·압축 해제
      ├─ 스키마 호환성 확인
      └─ 성공 시 원자적 활성 버전 교체
                   │
                   ▼
               SolidJS UI
```

프론트엔드는 R2나 공식 API를 직접 호출하지 않는다. 네트워크 요청, 서명·해시 검증, 압축 해제와 로컬 활성 버전 교체는 Rust 백엔드가 담당한다.

## 4. R2 접근 정책

### 4.1 개발과 운영 주소

- 개발 중에는 Cloudflare가 발급한 `r2.dev` Public Development URL을 사용한다.
- 공개 배포 전에는 `data.<project-domain>` 형태의 커스텀 도메인을 R2 버킷에 연결한다.
- 앱은 한 곳의 `DATA_BASE_URL`만 사용하고 모든 제어 문서와 asset 경로는 상대 경로로 저장한다.
- 커스텀 도메인 전환이 끝나면 `r2.dev` 공개 접근을 비활성화한다.
- 별도 Worker나 애플리케이션 서버는 초기 범위에 두지 않는다.

### 4.2 읽기와 쓰기 권한

- 앱 사용자는 공개 HTTPS 주소에서 읽기만 한다.
- S3 API endpoint, Access Key ID, Secret Access Key는 배포자와 GitHub Actions만 사용한다.
- R2 쓰기 자격증명과 Blizzard API 자격증명은 GitHub Actions Secret에만 둔다.
- 자격증명을 Tauri 바이너리, 프론트엔드, 공개 설정, R2 asset에 포함하지 않는다.

### 4.3 캐시 정책

| 객체 | Cache-Control 정책 |
|---|---|
| 서명이 포함된 `channels/stable.json` | 짧은 TTL과 재검증 |
| 서명이 포함된 `catalog/versions.json` | 짧은 TTL과 재검증 |
| 버전별 manifest와 서명 | `public, max-age=31536000, immutable` |
| 버전별 데이터·이미지팩 | `public, max-age=31536000, immutable` |

버전 경로의 파일은 덮어쓰지 않는다. 내용이 달라지면 새 버전 또는 revision을 발행한다.

커스텀 도메인의 일반 Cloudflare Cache 제한을 고려해 압축 asset 하나의 상한은 480 MiB로 둔다. 이를 넘는 asset만 결정론적으로 여러 조각으로 나눈다.

## 5. 버전 모델

데이터 버전은 공식 패치 번호, 확인된 build 식별자와 내부 revision을 조합한다.

```text
36.0.3-build247416-r1
```

| 부분 | 의미 |
|---|---|
| `36.0.3` | 공식 패치노트에서 얻은 패치 번호 |
| `build247416` | 확인된 공식 build 식별자 |
| `r1` | 같은 공식 상태를 다시 정제·배포한 내부 revision |

공식 패치 번호가 같아도 build가 다르면 별도 데이터 버전이다. 원본 수집이나 정규화 오류를 고친 경우 기존 파일을 덮어쓰지 않고 revision을 올린다.

### 5.1 세 종류의 제어 문서

```text
channels/stable.json
catalog/versions.json
releases/<data-version>/manifest.json
```

- `stable.json`: 현재 기본으로 설치할 최신 데이터 버전을 가리킨다.
- `versions.json`: 사용자가 열람할 수 있는 공식 패치 이벤트와 데이터 버전 목록을 제공한다.
- 버전별 `manifest.json`: 한 데이터 버전에 필요한 파일, 이미지 참조, 호환성과 해시를 상세히 기술한다.

R2 공개 버킷의 객체 목록을 앱에서 직접 조회하지 않는다.

### 5.2 카드 변경이 없는 패치

모든 공식 패치는 `versions.json`에 기록한다. API 카드 데이터 변화가 없으면 전체 스냅샷을 복제하지 않고 직전 스냅샷과 이미지 맵을 참조한다.

```text
36.0.2
└─ cardSnapshotRef: snapshot-abc

36.0.2.1
├─ observedChange: none
└─ cardSnapshotRef: snapshot-abc

36.0.3
└─ cardSnapshotRef: snapshot-def
```

## 6. 제어 문서 계약

아래 예시는 책임 경계를 고정하기 위한 최소 계약이다. 세부 필드 추가는 하위 호환 규칙을 따른다.

### 6.1 `channels/stable.json`

`stable.json`과 `versions.json`은 payload와 Ed25519 서명을 한 객체에 담은 signed envelope로 저장한다. 서명은 RFC 8785 방식으로 정규화한 payload 바이트를 대상으로 한다. 따라서 mutable 제어 문서와 서명 파일이 서로 다른 세대로 섞이는 문제가 없다. 아래 예시는 envelope 안의 payload다.

```json
{
  "formatVersion": 1,
  "currentDataVersion": "36.0.3-build247416-r1",
  "manifestPath": "releases/36.0.3-build247416-r1/manifest.json",
  "manifestSha256": "00770d0843f8034b96db7d4d9ad5f7da38a4e27fe7b57b2d0779cb4856167dd9",
  "publishedAt": "2026-07-25T00:00:00Z"
}
```

### 6.2 `catalog/versions.json`

```json
{
  "formatVersion": 1,
  "generatedAt": "2026-07-25T00:00:00Z",
  "versions": [
    {
      "officialPatchVersion": "36.0.3",
      "dataVersion": "36.0.3-build247416-r1",
      "manifestPath": "releases/36.0.3-build247416-r1/manifest.json",
      "cardSnapshotRef": "snapshot-def",
      "patchNoteStatus": "matched",
      "publishedAt": "2026-07-25T00:00:00Z"
    }
  ]
}
```

### 6.3 버전별 `manifest.json`

```json
{
  "schemaVersion": 1,
  "minimumAppVersion": "0.1.0",
  "dataVersion": "36.0.3-build247416-r1",
  "officialPatchVersion": "36.0.3",
  "buildId": 247416,
  "revision": 1,
  "supportedLocales": ["ko_KR", "en_US"],
  "locales": {
    "ko_KR": {
      "raw": {
        "path": "raw/ko_KR.json.zst",
        "bytes": 123,
        "sha256": "00770d0843f8034b96db7d4d9ad5f7da38a4e27fe7b57b2d0779cb4856167dd9",
        "defaultDownload": false
      },
      "normalized": {
        "path": "normalized/ko_KR.sqlite.zst",
        "bytes": 456,
        "sha256": "10770d0843f8034b96db7d4d9ad5f7da38a4e27fe7b57b2d0779cb4856167dd9",
        "defaultDownload": true
      },
      "diff": {
        "path": "diff/ko_KR.json.zst",
        "bytes": 78,
        "sha256": "20770d0843f8034b96db7d4d9ad5f7da38a4e27fe7b57b2d0779cb4856167dd9"
      },
      "imageMap": {
        "path": "images/ko_KR-map.json.zst",
        "bytes": 90,
        "sha256": "30770d0843f8034b96db7d4d9ad5f7da38a4e27fe7b57b2d0779cb4856167dd9"
      },
      "bootstrap": {
        "retention": "current-and-previous-for-7-days",
        "crop": [],
        "normal": []
      },
      "canonicalDeltas": []
    }
  },
  "imageIndex": {
    "path": "indexes/image-index-36.0.3-build247416-r1.json.zst",
    "bytes": 901,
    "sha256": "40770d0843f8034b96db7d4d9ad5f7da38a4e27fe7b57b2d0779cb4856167dd9"
  }
}
```

모든 asset 항목은 최소한 상대 경로, byte size, SHA-256을 가진다. 압축 해제 후 크기와 내부 파일 수 제한도 manifest에 기록해 압축 폭탄과 경로 탈출을 방지한다.

## 7. 원본과 정제 데이터

각 데이터 스냅샷은 원본과 정제본을 모두 R2에 보존한다.

### 7.1 원본

- 공식 API 응답을 의미 변경 없이 보존한다.
- 요청 locale, endpoint, 수집 시각과 응답 식별 정보를 함께 저장한다.
- 이미지 파일은 JSON 안에 포함하지 않고 공식 URL만 보존한다.
- 정규화 버그 수정, 재정제, 당시 공식 응답 검증에 사용한다.
- 일반 사용자 앱의 기본 다운로드 대상이 아니다.

### 7.2 정제본

- ID 기반으로 카드, 세트, 직업, 타입, 희귀도, 종족과 키워드를 조인한다.
- 빈 문자열과 비정상 표현을 안정된 앱 계약으로 변환한다.
- 관련 카드 관계, 제작·추출 비용과 검색용 평문을 구성한다.
- 버전별 완성된 SQLite 스냅샷으로 제공한다.
- 앱의 카드 목록, 상세, 필터와 고급 쿼리는 정제본만 사용한다.

사용자가 과거 버전을 열 때 diff를 처음부터 재생하지 않는다. 해당 버전의 완성된 정제 스냅샷을 바로 연다. diff는 변경 설명과 검증을 위한 부가 asset이다.

## 8. 이미지 모델

### 8.1 포함 범위

- 앱이 지원하는 locale마다 `normal`, `crop`을 저장한다.
- `gold` 이미지는 수집·배포 대상에서 제외한다.
- 초기 locale은 `ko_KR`, `en_US`다.
- 새 locale을 앱 지원 목록에 추가하면 카드 데이터, 이미지와 패치노트 수집 대상에도 함께 추가한다.

### 8.2 버전별 이미지 맵

각 카드 스냅샷은 해당 버전·locale·variant에서 사용한 실제 이미지 SHA-256을 기록한다.

```text
36.0.1 카드 A normal → hash-aaa
36.0.2 카드 A normal → hash-aaa
36.0.3 카드 A normal → hash-bbb
```

버전별 이미지 맵은 `cardId + locale + variant → content hash`를 제공한다. 정규화 DB에도 동일한 해시를 포함해 카드 조회와 이미지 조회가 일치하게 한다.

### 8.3 전역 이미지 인덱스

전역 인덱스는 이미지 해시가 어느 정본 압축팩의 어떤 member에 들어 있는지 제공한다.

```json
{
  "hash-aaa": {
    "pack": "canonical-images/initial/ko_KR-normal.tar.zst",
    "member": "hash-aaa.png"
  },
  "hash-bbb": {
    "pack": "canonical-images/36.0.3/ko_KR-normal-delta.tar.zst",
    "member": "hash-bbb.png"
  }
}
```

발행기는 이 인덱스를 사용해 이미 저장된 content hash를 변경팩에 다시 넣지 않는다. 각 버전 manifest는 발행 시점의 인덱스를 immutable 압축 asset으로 고정하고 그 SHA-256을 기록한다. 과거 버전 조회도 해당 버전이 고정한 인덱스를 사용하므로, 이후 인덱스가 확장되어도 기존 해석이 바뀌지 않는다.

같은 이미지 URL이어도 실제 바이트가 다르면 다른 해시다. URL이 달라도 바이트가 같으면 기존 해시를 재사용한다.

### 8.4 이미지 정본팩

이미지 정본은 최초 기준팩과 패치별 변경팩으로 구성한다.

```text
canonical-images/
├─ initial/
│  ├─ ko_KR-normal.tar.zst
│  └─ ko_KR-crop.tar.zst
└─ 36.0.3-build247416-r1/
   ├─ ko_KR-normal-delta.tar.zst
   └─ ko_KR-crop-delta.tar.zst
```

변경팩에는 전역 이미지 인덱스에 없는 새 content hash만 넣는다. 변경되지 않은 카드는 이전 정본팩의 해시를 직접 참조한다.

카드 이미지 한 장을 여러 조각으로 자르지 않는다. 480 MiB를 넘는 압축팩만 여러 archive로 나누고 각 PNG는 하나의 archive member로 유지한다.

### 8.5 최신 전체 설치팩

신규 사용자와 새 locale 설치 편의를 위해 현재 이미지 맵을 완전히 반영한 전체 설치팩을 별도로 만든다.

```text
bootstrap/<image-snapshot-version>/<locale>/
├─ crop-current-000.tar.zst
└─ normal-current-000.tar.zst
```

- 신규 설치는 정본 변경팩 체인을 받지 않고 최신 전체 설치팩을 받는다.
- 기존 설치는 새 패치의 정본 변경팩만 받는다.
- 과거 버전 열람은 정본 이미지 인덱스를 사용한다.
- 최신 전체 설치팩은 파생 asset이며 역사 보관의 정본이 아니다.
- 현재 이미지 맵이 변하지 않은 데이터 버전은 기존 최신 설치팩을 재사용한다.
- 새 설치팩을 안정적으로 게시한 뒤 직전 설치팩을 7일간 유지하고 삭제한다.
- 정상 상태에서는 정본 이미지와 현재 전체 설치팩 한 벌의 의도적인 중복만 허용한다.
- 과거 버전 조회는 manifest가 고정한 이미지 맵과 정본 인덱스를 사용한다. 만료 가능한 bootstrap 경로를 과거 버전 복구의 필수 의존성으로 사용하지 않는다.

## 9. 카드 JSON을 이용한 이미지 1차 필터

API 변경을 확인할 때 지원 locale의 카드 JSON과 메타데이터는 전체 수집해 이전 스냅샷과 비교한다. 이미지는 전체 수집하지 않는다.

```text
전체 카드 JSON 수집
→ 정규화 전 원본 fingerprint 비교
→ 카드별 필드 diff
→ 이미지에 영향을 주는 카드·variant만 선별
→ 필요한 이미지만 조건부 요청 또는 다운로드
→ 실제 SHA-256이 새로울 때만 정본 변경팩에 추가
```

기본 선별 규칙은 다음과 같다.

| JSON 변경 | 이미지 처리 |
|---|---|
| 이름, 카드 효과, 마나, 공격력, 생명력, 내구도 | 해당 locale의 normal 확인 |
| normal 이미지 URL | normal 확인 |
| crop 이미지 URL 또는 카드 아트 식별자 | crop 확인 |
| 카드 프레임에 영향을 주는 직업·타입·희귀도 | normal 확인 |
| 관련 카드 ID, 수집 가능 여부, 아티스트, 플레이버 텍스트 | URL이 같으면 이미지 확인 생략 |

한 locale의 텍스트만 바뀌면 해당 locale의 normal만 확인한다. 카드 텍스트 변경만으로 crop을 다시 받지 않는다.

이미지 URL과 JSON이 모두 그대로인 상태에서 CDN이 같은 URL의 바이트만 교체하는 경우는 일반 패치 diff로 발견할 수 없다. 매 패치마다 전체 이미지를 재다운로드하지 않으며, 이 예외는 수동 무결성 검사에서 조건부 요청으로 확인한다.

## 10. 공식 패치노트 수집

공식 Game Data API는 패치노트나 과거 카드 상태를 제공하지 않는다. 공식 Hearthstone 뉴스 페이지의 패치 게시물을 별도 어댑터로 수집한다.

### 10.1 저장 범위

- 패치 article ID, 공식 패치 번호, 제목, 게시일과 URL
- 앱이 지원하는 모든 locale의 공식 페이지
- HTML에서 추출한 카드별 변경 전·후 사실
- 원문 fetch 시각과 원문 body SHA-256
- API 카드와의 연결 상태

전체 HTML과 게시물 장식 이미지, 영상 썸네일과 전장 스크린샷은 R2 배포 asset으로 보존하지 않는다. 카드 이미지는 카드 이미지 파이프라인에서만 저장한다.

초기 지원 locale은 `ko_KR`, `en_US`다. 특정 locale의 패치노트가 없으면 `en_US`를 화면 fallback으로 사용한다. 패치노트 locale 누락은 카드 데이터 발행을 막지 않는다.

### 10.2 API diff와 분리

```text
API snapshot diff
└─ 실제 공식 API 데이터가 어떻게 달라졌는지

patch-note reported changes
└─ Blizzard가 패치노트에서 무엇을 변경했다고 발표했는지
```

패치노트 파서 결과가 API diff를 덮어쓰지 않는다.

| 상태 | 의미 |
|---|---|
| `matched` | 공식 card ID로 API 카드와 확실히 연결됨 |
| `unmatched` | 연결 가능한 카드를 찾지 못함 |
| `ambiguous` | 후보가 여러 개라 자동 확정하지 않음 |
| `unannounced_api_change` | 패치노트에 없지만 API에서 변경됨 |
| `patch_api_mismatch` | 패치노트의 예상 변경이 API에서 확인되지 않음 |

## 11. 패치 탐지와 발행 시점

패치노트는 패치 이벤트와 예상 변경을 알려주는 트리거다. 실제 데이터 반영 완료는 Game Data API로 확인한다.

```text
공식 패치노트 발견
→ patch event: announced
→ 예상 카드 변경 추출
→ Game Data API 전체 카드 JSON 확인
→ 실제 변경 확인
→ 수집·정규화·이미지 선별·검증
→ 즉시 R2 발행
```

여기서 즉시는 변경을 보자마자 검증 없이 공개한다는 뜻이 아니다. 지원 locale 전체의 수집, diff, asset 생성, 서명과 검증을 모두 통과한 직후 발행한다.

패치 적용 예정 시각부터 24시간 동안의 처리 규칙은 다음과 같다. 패치노트에서 적용 시각을 확인할 수 없으면 공식 게시 시각을 기준으로 사용하고 그 사실을 patch event에 기록한다.

| 패치노트 | API 상태 | 처리 |
|---|---|---|
| 카드 변경 발표 있음 | 예상 변경 확인 | 새 스냅샷 즉시 발행 |
| 카드 변경 발표 있음 | 24시간 후에도 미확인 | `patch_api_mismatch`, 자동 무변경 확정 금지 |
| 카드 변경 발표 없음 | 24시간 동안 변화 없음 | 이전 스냅샷을 재사용하는 패치 이벤트 발행 |
| 내용과 무관하게 API 변경 발견 | diff 존재 | 새 스냅샷 발행, 필요하면 `unannounced_api_change` |

24시간 이후에도 주기적 감시는 계속한다. 늦은 API 변경이 발견되면 같은 공식 패치에 새 build 또는 revision 데이터 버전을 연결한다. 수동 `workflow_dispatch` 경로를 항상 유지한다.

## 12. 전자서명과 무결성

파일별 SHA-256만으로는 R2 제어 권한을 탈취한 공격자가 파일과 해시를 함께 바꾸는 상황을 막을 수 없다.

CI는 다음 제어 문서를 비공개 서명 키로 서명한다.

- `channels/stable.json`
- `catalog/versions.json`
- 버전별 `manifest.json`

mutable 문서인 `stable.json`과 `versions.json`은 payload와 서명을 한 파일에 담은 signed envelope다. immutable 문서인 버전별 `manifest.json`은 옆에 `manifest.sig`를 둔다. stable payload가 manifest 경로와 SHA-256을 함께 서명하므로 다른 세대의 manifest나 서명으로 바꿔치기할 수 없다. Tauri 앱에는 검증용 공개 키만 포함한다.

```text
제어 문서 서명 검증
→ manifest의 dataVersion·호환성 확인
→ asset 다운로드
→ 압축 파일 SHA-256 확인
→ 압축 해제 제한·내부 파일 해시 확인
→ 스키마 검증
→ 활성 버전 교체
```

앱은 서명에 실패한 문서, manifest와 SHA-256이 다른 asset, 로컬 버전보다 오래된 stable 버전을 자동 설치하지 않는다. 과거 버전 열람은 사용자가 명시적으로 선택한 경우에만 허용한다.

서명 private key는 GitHub Actions Secret에만 저장한다. 키 교체 시 새 공개 키를 신뢰하는 앱 버전을 먼저 배포한 뒤 데이터 서명 키를 전환한다.

## 13. 앱 다운로드 흐름

### 13.1 첫 실행

기본 locale은 `ko_KR`다.

```text
stable·manifest 서명 검증
→ ko_KR 정제 카드 DB 다운로드·검증
→ 카드 검색과 필터 사용 가능
→ ko_KR crop 전체 설치팩 우선 다운로드
→ ko_KR normal 전체 설치팩 백그라운드 다운로드
```

이미지가 아직 없는 카드는 플레이스홀더를 표시하고 다운로드 완료 시 실제 이미지로 교체한다.

### 13.2 locale 전환

- 현재 선택한 locale만 기본 다운로드한다.
- 사용자가 다른 지원 locale로 처음 전환할 때 해당 정제 DB와 이미지 설치팩을 받는다.
- 이미 받은 locale은 로컬 캐시를 재사용한다.
- 지원하지 않는 locale의 데이터와 이미지를 미리 받지 않는다.

### 13.3 기존 설치 업데이트

```text
현재 dataVersion과 stable 비교
→ 새 정제 카드 스냅샷 다운로드
→ 현재 이미지 맵과 새 이미지 맵 비교
→ 필요한 정본 delta만 다운로드
→ 임시 경로에서 검증·압축 해제
→ 성공 시 원자적으로 활성 버전 전환
```

기존 사용자는 최신 전체 설치팩을 다시 받지 않는다.

### 13.4 과거 버전 열람

```text
versions.json에서 버전 선택
→ 선택 버전의 완성된 정제 스냅샷 로드
→ 카드별 이미지 hash 확인
→ 로컬 캐시에 있으면 즉시 표시
→ 없으면 전역 이미지 인덱스가 가리키는 정본팩 다운로드
```

과거 버전 조회를 위해 diff 체인을 런타임에 재생하지 않는다.

## 14. 호환성과 실패 처리

### 14.1 스키마 호환성

manifest는 `schemaVersion`과 `minimumAppVersion`을 가진다.

- 현재 앱이 스키마를 읽을 수 있으면 데이터 업데이트를 진행한다.
- 읽을 수 없으면 새 데이터를 설치하지 않고 기존 데이터를 유지한다.
- UI에는 앱 업데이트가 필요하다는 메시지와 Tauri 앱 업데이트 진입점을 제공한다.
- 구형 앱용 정제 데이터를 별도로 계속 생성하지 않는다.

### 14.2 업데이트 실패

기존 데이터가 있는 경우:

- 카드 라이브러리를 기존 데이터로 계속 사용한다.
- 비차단 알림과 `다시 시도` 버튼을 표시한다.
- 부분 다운로드를 활성 데이터로 승격하지 않는다.

최초 실행이라 기존 데이터가 없는 경우:

- 데이터가 필요하다는 초기 상태 화면을 표시한다.
- 네트워크 상태와 재시도 동작을 제공한다.
- 손상되거나 서명되지 않은 데이터를 임시로 열지 않는다.

정확한 이미지 해시를 받지 못했을 때 이전 버전 이미지를 대신 표시하지 않는다. 버전과 다른 이미지를 보여주지 않고 플레이스홀더와 재시도 상태를 사용한다.

## 15. 발행 트랜잭션

R2는 여러 객체를 하나의 트랜잭션으로 공개하지 않으므로 pointer-last 규칙을 사용한다.

```text
1. 새 버전의 immutable asset 업로드
2. 업로드된 byte size, SHA-256과 최신 전체 설치팩 완전성 재검증
3. 버전 manifest와 서명 업로드
4. 서명된 versions envelope 갱신
5. 서명된 stable envelope를 마지막에 갱신
6. 직전 설치팩 7일 보존 후 정리
```

1~4단계에서 실패하면 stable을 바꾸지 않는다. 사용자는 이전 완성 버전을 계속 사용한다.

롤백은 이전에 서명된 manifest를 가리키는 새 signed stable envelope를 발행하는 방식으로 수행한다. 버전 경로의 기존 파일을 수정하지 않는다.

## 16. 로컬 저장과 R2 배포 원본의 구분

```text
Cloudflare R2
└─ 새 설치·업데이트·과거 버전 복구에 사용하는 배포 원본

사용자 장치의 로컬 이미지 캐시
└─ R2 asset에서 다시 생성할 수 있는 장치별 결과
```

R2 정본팩의 압축 형태와 로컬 캐시의 개별 이미지 형태를 동일하게 만들 필요는 없다.

```text
R2
└─ 여러 PNG를 담은 tar.zst

로컬
└─ {cache}/images/{locale}/{variant}/{sha256}.png
```

앱 설치형과 포터블 모드 사이의 설정·DB·로컬 캐시 이전 정책은 별도 포터빌리티 설계를 따르며, R2 정본이나 최신 설치팩 보존 정책과 결합하지 않는다.

## 17. 검증 계획

### 17.1 fixture 테스트

- 카드 추가, 수정, 삭제와 재등장
- JSON은 같고 URL만 바뀐 이미지
- URL은 다르지만 바이트 해시가 같은 이미지
- 한 locale의 텍스트만 변경
- normal만 변경되고 crop은 유지
- 관련 카드·collectible만 변경되어 이미지 검사를 생략하는 경우
- 패치노트 변경과 API diff 일치·불일치·모호성
- 카드 변경 없는 공식 패치의 스냅샷 재사용

### 17.2 패키징 테스트

- 동일 이미지 hash가 정본팩 두 곳에 들어가지 않음
- bootstrap이 현재 이미지 맵을 완전히 포함
- 480 MiB 상한에서 결정론적 분할
- archive member 경로 탈출 차단
- 압축 해제 후 파일 수·총 크기 제한

### 17.3 updater 테스트

- 서명 성공·실패와 알 수 없는 key
- asset SHA-256 불일치
- 네트워크 중단과 재시도
- 부분 다운로드 후 기존 데이터 유지
- 지원하지 않는 schema와 minimum app version
- stable 전환 전후의 원자성
- 선택한 과거 버전의 정확한 이미지 hash 표시

### 17.4 R2 smoke test

개발용 `r2.dev` URL에 SHA-256을 파일명으로 사용한 PNG를 업로드해 다음을 확인했다.

- 공개 GET은 HTTP 200을 반환했다.
- 응답 Content-Type은 `image/png`였다.
- 다운로드한 파일 SHA-256과 객체 파일명이 일치했다.

이 smoke test는 공개 전달 경로 확인용이며 최종 배포 구조의 개별 이미지 다운로드 정책을 의미하지 않는다.

## 18. 구현 경계

이 문서는 구현을 시작하지 않는다. 구현은 다음 독립 단위로 나눌 수 있다.

1. 공식 API·패치노트 수집기와 fixture
2. 원본·정제 스냅샷과 diff 생성기
3. 이미지 후보 선별, 해시 인덱스와 정본팩 생성기
4. R2 발행기, 전자서명과 pointer-last 배포
5. Rust `CardDataUpdater`와 로컬 원자적 교체
6. SolidJS 시작·진행·실패·과거 버전 상태

각 단위의 상세 작업 순서와 테스트 파일은 별도 구현 계획에서 확정한다.

## 19. 공식 근거

- Blizzard Hearthstone Game Data API: https://develop.battle.net/documentation/hearthstone/game-data-apis
- Blizzard Developer API Terms of Use: https://www.blizzard.com/en-us/legal/a2989b50-5f16-43b1-abec-2ae17cc09dd6/blizzard-developer-api-terms-of-use
- Hearthstone 공식 뉴스: https://hearthstone.blizzard.com/ko-kr/news
- Cloudflare R2 pricing: https://developers.cloudflare.com/r2/pricing/
- Cloudflare R2 public buckets: https://developers.cloudflare.com/r2/buckets/public-buckets/
- Cloudflare R2 cache: https://developers.cloudflare.com/cache/interaction-cloudflare-products/r2/
- Cloudflare R2 object lifecycle: https://developers.cloudflare.com/r2/buckets/object-lifecycles/
- Tauri updater: https://v2.tauri.app/ko/plugin/updater/
