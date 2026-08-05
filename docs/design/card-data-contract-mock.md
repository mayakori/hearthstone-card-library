# Card data contract mock

Status: Draft for schema review<br>
Captured: 2026-07-18 KST<br>
Locale: `ko_KR`

이 문서는 공식 카드 라이브러리에서 2026-07-18에 직접 가져온 응답을 원본, 디스크 캐시, 프론트 IPC의 세 층으로 재구성한 설계 목업이다. 기계 판독 가능한 전체 샘플은 [`card-data-contract-mock.ko-KR.json`](./card-data-contract-mock.ko-KR.json)에 있다.

## 확인된 사실

- 카드 목록은 `/{urlLocale}/api/cards`에서 JSON으로 내려온다.
- `locale=ko_KR` 쿼리를 넣으면 `name`, `text`, `image`가 한국어 문자열로 내려온다. 이 값이 없으면 `name`과 `image`가 locale별 객체로 내려오므로 원본 디코더는 두 형태를 모두 허용해야 한다.
- `textFilter=공허&set=escape-from-violet-hold`는 현재 5장을 반환했다.
- 카드 상세용 별도 `/api/cards/{slug}` 엔드포인트는 404였다. 현재 목록 객체가 설명, 이미지, 관계 ID 등 상세 필드를 이미 포함하므로 v1에서는 별도 상세 fetch가 필요 없다.
- 메타데이터는 별도 JSON API가 아니라 카드 라이브러리 HTML의 `#cardGalleryMount` `config` 속성에 들어 있다.
- `온천 활공꾼` 카드 객체에는 `artistName`, `flavorText`, `keywordIds`가 직접 포함되어 있다. 종류·종족·등급·세트·직업 이름은 ID로 메타데이터를 조인한다.
- 제작 비용과 추출 가루는 카드 객체가 아니라 희귀도 메타데이터의 `craftingCost`, `dustValue`에서 가져온다. 일반 등급은 현재 각각 `[40, 400]`, `[5, 50]`이다.
- 키워드 상세 설명은 카드의 `keywordIds`와 메타데이터 `keywords`를 조인한다. `온천 활공꾼`은 천상의 보호막(3), 전투의 함성(8), 유사(351)를 가진다.
- `불카노스` 원본은 `childIds=[123666, 128032]`를 제공한다. 두 ID는 `ids=123666,128032` 묶음 요청 한 번으로 조회할 수 있다.
- 두 `불카노스의 융기`는 모두 `collectible=0`, `parentId=123665`다. 이름·효과·능력치는 같지만 공식 ID와 이미지가 다르므로 이름으로 중복 제거하지 않는다.
- 관련 카드도 일반 카드와 같은 정규화 모델에 저장한다. 부모 `CardDetail`에서는 `relations.children: CardSummary[]`로 조립하고, 원본 `childIds`와 자식 `parentId`는 별도로 보존한다.
- `불카노스`의 키워드 231은 메타데이터에서 `거수 +X`와 설명 `소환하면, 부위를 X개 생성합니다.`로 조인된다.
- 실데이터의 전투의 함성 slug는 끝에 개행이 붙은 `"battlecry\n"`이었다. raw에는 그대로 두고 정규화 어댑터에서 trim한다.
- 신규 카드의 `imageGold`는 빈 문자열일 수 있다. 정규화 계층에서는 빈 문자열을 `null`로 바꾼다.
- 바쿠의 일반, 크롭, 황금 이미지 URL은 모두 HTTP 200이었다. 황금 이미지는 `image/png`, 98,560바이트였으며 열람 시 다운로드하는 흐름을 구성할 수 있다.
- 공식 응답의 `공허의 영혼` 텍스트는 현재 `비용이 인 ...`처럼 일부 값이 빠져 있다. 원본을 고치지 말고 그대로 보존하며, 추론이나 보정은 별도 필드로 둬야 한다.

## 세 층의 책임

```text
공식 홈페이지
  ├─ 카드 JSON 응답
  └─ HTML 내 메타데이터
          │
          ▼
Raw cache
  ├─ URL / locale / ETag / fetched_at
  └─ 공식 응답을 변형 없이 보존
          │
          ▼
Normalized cache + memory catalog
  ├─ ID 기반 정규화
  ├─ 빈 문자열 → null
  ├─ HTML 효과 문구 → plain + markup
  └─ 세트별 JSON shard
          │
          ▼
Tauri IPC
  ├─ CardSummary / CardDetail
  ├─ CardQuery / CardPage
  ├─ MetadataCatalog
  ├─ LibraryStatus / SyncStatus
  └─ ImageRequest / ImageAsset
```

## IPC 스키마

| 스키마 | 용도 | 주요 규칙 |
|---|---|---|
| `MetadataCatalog` | ID를 한글 이름과 slug로 표시 | 종족·주문 계열, 희귀도별 제작/추출 값, 키워드 설명까지 보존한다. |
| `CardSummary` | 카드 그리드와 검색 결과 | 평문 효과만 포함하고 상세 전용 필드는 제외한다. |
| `CardDetail` | 상세 패널과 관련 카드 탐색 | `CardSummary`와 함께 taxonomy, economy, 해석된 keyword 객체를 넣는다. 관계 ID가 있으면 관련 카드 `CardSummary`까지 채워 단독으로 상세 화면을 그릴 수 있게 한다. |
| `CardQuery` | AND/OR 고급 검색 | 중첩 가능한 group/condition 트리이며 cursor는 불투명 문자열이다. |
| `CardPage` | 검색 결과 페이지 | `from_cache`와 `stale`을 포함해 SWR 동작을 프론트가 구분한다. |
| `LibraryStatus` | 앱 시작 상태 | 현재 catalog가 표시 가능한지와 마지막 성공 시각을 제공한다. |
| `SyncStatus` | 동기화 진행 이벤트 | 단계, 페이지 수, 발견 카드 수, 이미지 진행량을 제공한다. |
| `ImageRequest` | 이미지 확보 요청 | `normal`, `crop`, `gold` 중 하나를 요청한다. |
| `ImageAsset` | 로컬 이미지 결과 | 절대 경로 대신 cache key와 앱 기준 상대 경로를 반환한다. |

### CardQuery 예시

```json
{
  "locale": "ko_KR",
  "where": {
    "kind": "group",
    "operator": "and",
    "children": [
      { "kind": "condition", "field": "set_id", "operator": "eq", "value": 1988 },
      {
        "kind": "group",
        "operator": "or",
        "children": [
          { "kind": "condition", "field": "name", "operator": "contains", "value": "공허" },
          { "kind": "condition", "field": "text_plain", "operator": "contains", "value": "공허" }
        ]
      }
    ]
  },
  "sort": [{ "field": "date_added", "direction": "desc" }],
  "cursor": null,
  "limit": 24
}
```

이 조건은 실제 공식 검색 결과와 동일하게 5장으로 목업했다. `CardPage`의 전체 샘플은 JSON 파일의 `ipc_wire_mocks.card_page`에 있다.

### 온천 활공꾼 상세 예시

```text
Official card object
  ├─ name / text / flavorText / artistName
  ├─ classId=5 / cardTypeId=4 / minionTypeId=14
  ├─ cardSetId=1952 / rarityId=1
  └─ keywordIds=[3, 8, 351]
             +
#cardGalleryMount config
  ├─ 성기사 / 하수인 / 멀록 / 운고로의 잃어버린 도시 / 일반
  ├─ craftingCost=[40, 400] / dustValue=[5, 50]
  └─ 천상의 보호막 / 전투의 함성 / 유사 설명
             ↓
CardDetail
  ├─ taxonomy: 해석된 분류 객체
  ├─ economy: 일반·황금 제작 및 추출 값
  └─ keywords: 이름과 설명이 포함된 객체 3개
```

전체 원본은 `official_raw_samples.hot_spring_glider`, 조인에 사용한 메타데이터는 `official_raw_samples.hot_spring_glider_metadata_join`, 프론트 계약 결과는 `ipc_wire_mocks.card_detail`에서 확인한다.

### 불카노스 관련 카드 예시

```text
OfficialCard #123665 불카노스
  └─ childIds=[123666, 128032]
              │
              ├─ GET /api/cards?...&ids=123666,128032
              │
              ├─ OfficialCard #123666 불카노스의 융기
              │    ├─ collectible=0
              │    ├─ parentId=123665
              │    └─ image=fa51...png
              │
              └─ OfficialCard #128032 불카노스의 융기
                   ├─ collectible=0
                   ├─ parentId=123665
                   └─ image=8192...png
                              ↓
CardDetail #123665
  └─ relations
       ├─ child_ids=[123666, 128032]
       └─ children=[CardSummary #123666, CardSummary #128032]
```

프론트는 별도 IPC를 두 번 호출하지 않고 `ipc_wire_mocks.vulcanos_card_detail` 하나로 본체와 관련 카드 두 장을 그린다. 원본 본체는 `official_raw_samples.vulcanos`, 자식 응답은 `official_raw_samples.vulcanos_related_cards`, 분류·희귀도·거수 설명 조인은 `official_raw_samples.vulcanos_metadata_join`에서 확인한다.

### 황금 이미지 예시

```text
상세 열기
  → CardDetail.image.gold.state == remote
  → ensure_card_image({ card_id: 48158, variant: gold })
  → CDN 다운로드 + atomic write
  → ImageAsset { state: ready, downloaded_now: true }
  → 이후 요청은 같은 cache key로 즉시 반환
```

## 디스크 배치 목업

```text
<app_cache_dir>/card-data/
├─ manifest.json
├─ raw/
│  └─ ko_KR/cards/the-lost-city-of-ungoro/page-0001.json
├─ normalized/
│  └─ ko_KR/
│     ├─ metadata.json
│     └─ sets/1952.json
└─ images/
   └─ ko_KR/
      ├─ normal/<card-id>.png
      ├─ crop/<card-id>.png
      └─ gold/<card-id>.png
```

`manifest.json`과 shard는 임시 파일에 쓴 뒤 `fsync`와 rename으로 교체한다. 포맷 버전이 다르거나 JSON이 손상되면 재수집 가능한 캐시로 취급해 폐기한다. 새 catalog는 모든 page와 shard가 완성된 뒤에만 `active_catalog`로 승격한다.

## 아직 고정하지 않은 부분

- `text_markup`을 프론트에서 allowlist sanitize할지, Rust에서 rich-text segment로 바꿀지
- cursor의 실제 인코딩 방식
- 이미지 상대 경로를 Tauri asset URL로 바꾸는 최종 어댑터 형식
- 전체 정규 카드 1,152장의 일반·크롭 이미지를 첫 동기화에서 모두 받을지, 카드 데이터부터 표시하고 이미지 다운로드를 별도 단계로 할지

이 네 항목은 wire 필드 이름을 바꾸지 않고도 구현 정책을 교체할 수 있게 목업했다.
