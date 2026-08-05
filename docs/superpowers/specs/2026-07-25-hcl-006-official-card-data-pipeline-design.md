# HCL-006 공식 Hearthstone 카드 데이터 파이프라인 구현 명세

Status: Approved
Updated: 2026-08-05 KST
Scope: Blizzard 공식 API 수집, Raw 보존, locale별 SQLite 정규화, 결정론적 로컬 패키징

## 1. 목적과 정본

이 명세는 Blizzard 공식 Hearthstone Game Data API에서 현재 정규전 카드 데이터를 수집해 `ko_KR`, `en_US` Raw 스냅샷과 완성된 SQLite 스냅샷을 만드는 Rust CLI의 구현 계약이다.

데이터 정본은 Blizzard 공식 Game Data API다. 과거 공식 홈페이지 내부 `/api/cards`, HTML 메타데이터, 미리보기 mock은 탐색 기록일 뿐 운영 수집 계약이 아니다. 이 문서와 `docs/design/card-data-architecture-decisions.md`가 충돌하면 HCL-006 구현 세부사항에는 이 문서를 적용한다.

### 1.1 결정 문서 우선순위

HCL-006을 구현할 때 문서는 다음 순서로 해석한다.

1. 이 문서: HCL-006 수집·정규화·로컬 패키징 구현 계약
2. `docs/design/card-data-architecture-decisions.md`: 확정된 제품·데이터 아키텍처
3. `docs/design/card-data-r2-release-design.md`: 이 문서가 아직 다루지 않는 후속 R2 배포 계약
4. 기존 mock과 explorer: 실데이터 사례와 필드 의미를 확인하는 역사적 목업
5. `docs/handoff-hcl-006-official-api-pipeline.md`: 명세 세션을 시작한 시점의 역사적 핸드오프

하위 문서의 과거 endpoint, 전체 constructed 카드 범위, JSON 정규화 형식과 gold 이미지 예시는 이 문서를 덮어쓰지 않는다. 특히 핸드오프의 game mode 선택 질문은 이후 대화에서 `current Standard`로 확정됐으므로 다시 선택하지 않는다. 기존 mock의 `/{urlLocale}/api/cards`와 HTML 메타데이터 수집도 폐기된 탐색 경로다.

### 1.2 다시 선택하지 않는 잠금 결정

- 수집 정본은 Blizzard 공식 Hearthstone Game Data API다.
- 기본 범위는 `ko_KR`, `en_US`의 현재 정규전 카드이며, 연결된 비수집 카드와 기본 영웅·영웅 능력은 같은 카드 모델에 보존한다.
- locale별 Raw JSON과 locale별 완성 SQLite를 각각 만든다.
- 이름이 같아도 공식 카드 ID가 다르면 합치지 않는다.
- 공식 텍스트는 표시용 markup과 검색용 plain을 분리한다.
- 룬과 sideboard는 현재 공식 API의 선택적 1:1 객체이므로 별도 table로 쪼개지 않고 `cards`의 묶음 nullable column으로 보존한다.
- 공식 관계는 원본 필드와 배열 순서를 보존한다. `child`·`bundled` target은 build validation으로 존재를 강제하고, 범위 밖일 수 있는 `parent`·`copy_of` 때문에 모든 target에 FK를 걸지는 않는다.
- SQLite FTS와 초성 전용 column을 만들지 않는다. 후속 Tauri Rust 백엔드가 SQLite를 메모리 카탈로그로 읽어 검색한다.
- JSONL 로그에는 stage, retry, locale 요약과 최종 결과만 기록하고 모든 성공 HTTP 요청을 나열하지 않는다.
- 첫 manifest 상수는 `schemaVersion = 1`, `minimumAppVersion = "0.1.0"`이다.

첫 구현은 다음 수직 절편에서 끝난다.

```text
Blizzard OAuth
→ ko_KR·en_US 현재 정규전 카드와 메타데이터 수집
→ locale별 canonical Raw JSON
→ locale별 정규화 SQLite
→ zstd 압축
→ SHA-256과 manifest
→ 로컬 output 폴더
```

## 2. 범위 밖

첫 구현에는 다음을 포함하지 않는다.

- GitHub Actions workflow와 Cloudflare R2 업로드
- 전자서명, `stable.json`, `versions.json`
- 이미지 다운로드와 이미지 팩
- 패치노트 파싱과 버전 diff
- Tauri updater, IPC, SolidJS 화면
- SQLite FTS, 초성 컬럼과 검색 엔진
- 카드 효과 의미 추론과 개인 카드풀
- 공식 텍스트의 locale 간 보정

검색은 후속 Tauri Rust 백엔드가 locale SQLite를 메모리 카탈로그로 읽은 뒤 실행한다. HCL-006 SQLite는 영구 저장과 무결성의 정본이며 검색 전용 중복 컬럼을 갖지 않는다.

## 3. Rust 책임 경계

같은 저장소의 Rust workspace에 다음 crate를 둔다.

```text
crates/
├─ card-data-contract/   # Raw, 정규화, manifest 공용 타입과 검증 규칙
└─ card-data-pipeline/   # OAuth, 수집, 정규화, SQLite, 압축, CLI
```

`card-data-pipeline`은 로컬과 향후 CI에서만 실행하며 Tauri 설치 파일에 포함하지 않는다. 구체적인 모듈 파일 배치는 구현 계획에서 정하되 수집 어댑터, 정규화 계약과 패키징 책임을 서로 침범하지 않는다.

## 4. CLI와 비밀값

공개 명령은 하나다.

```powershell
card-data-pipeline build `
  --data-version 36.0.3-build247416-r1 `
  --output-root .\output
```

`--data-version`은 `^[0-9]+\.[0-9]+\.[0-9]+-build[1-9][0-9]*-r[1-9][0-9]*$`를 만족해야 한다. CLI는 여기서 `officialPatchVersion`, `buildId`, `revision`을 파싱하므로 중복 인자를 받지 않는다.

출력 경로는 `<output-root>/<data-version>/`이다. 같은 data version 경로가 이미 존재하면 덮어쓰지 않고 exit 2로 종료한다. locale 부분 실행, 기존 버전 교체, append 모드는 제공하지 않는다.

인증 입력은 다음 process environment만 허용한다.

```text
BLIZZARD_CLIENT_ID
BLIZZARD_CLIENT_SECRET
```

로컬 개발자는 Git에서 무시되는 `.env.card-data.local`을 process environment로 불러올 수 있지만 파이프라인이 파일 내용을 출력하거나 산출물에 복사해서는 안 된다. 누락된 자격증명은 CLI/config 오류다.

## 5. OAuth 계약

- token endpoint: `POST https://oauth.battle.net/token`
- grant: `client_credentials`
- access token은 process memory에만 둔다.
- 토큰, client ID, client secret과 Authorization header를 파일, Raw, SQLite, manifest, 로그에 기록하지 않는다.
- 만료 5분 전이면 다음 요청 전에 갱신한다.
- API가 401을 반환하면 토큰을 한 번 폐기·갱신하고 해당 요청을 한 번 다시 보낸다.
- 갱신 뒤에도 401이면 OAuth 오류로 종료한다.
- pipeline process가 끝나면 token 상태도 사라진다.

## 6. 공식 API 수집 범위

API region은 `us`로 고정한다. locale은 `ko_KR`, `en_US` 두 개를 모두 수집한다.

### 6.1 기본 카드 목록

endpoint:

```text
GET https://us.api.blizzard.com/hearthstone/cards
```

고정 query:

```text
locale=<ko_KR|en_US>
set=standard
gameMode=constructed
collectible=0,1
pageSize=500
page=<1-based page>
```

페이지는 1부터 `pageCount`까지 순차 요청한다. 서버 기본 page size에 의존하지 않는다. 현재 공식 응답에서 `pageSize=500`은 유효하며 2026-08-05 기준 1,559장을 네 페이지로 반환했다.

같은 live 응답에서 Standard 카드 객체에 나타난 33개 필드는 이 명세의 typed column·연결 table·relation 또는 Raw-only 정책으로 모두 대응된다. 장소 34장은 모두 `health`, 무기 46장도 모두 `health`를 반환했고 별도 `durability` key는 나타나지 않았다. 따라서 HCL-006은 공식 `health`를 그대로 보존하며 타입별 의미를 재해석하거나 관측되지 않은 `durability` column을 만들지 않는다. 이 개수와 필드 부재는 2026-08-05 관측값이지 schema 상수가 아니다.

각 locale에서 다음을 검증한다.

- 응답 `page`가 요청 page와 같다.
- 모든 page의 `pageCount`, `cardCount`가 같다.
- page가 1부터 마지막까지 빠짐없이 한 번씩 존재한다.
- 합친 카드 수가 `cardCount`와 같다.
- 기본 카드 ID에 중복이 없다.

### 6.2 메타데이터

endpoint:

```text
GET https://us.api.blizzard.com/hearthstone/metadata?locale=<locale>
```

다음 일곱 배열은 키와 배열 타입이 반드시 존재해야 한다.

```text
sets
classes
types
rarities
minionTypes
spellSchools
keywords
```

고정 개수와 알려진 ID allowlist는 두지 않는다. `setGroups`, `gameModes`, `bgGameModes`, Mercenary 관련 배열과 미래 top-level 필드는 Raw에는 그대로 남지만 첫 정규화 스키마에는 넣지 않는다.

### 6.3 관련 카드 closure

기본 카드에 나타난 다음 forward relation target이 기본 집합에 없으면 개별 카드 endpoint로 가져온다.

```text
childIds
bundledCardIds
```

endpoint:

```text
GET https://us.api.blizzard.com/hearthstone/cards/<card-id>?locale=<locale>
```

새로 받은 카드의 `childIds`, `bundledCardIds`도 같은 방식으로 재귀 확인해 closure가 닫힐 때까지 수집한다. 요청 ID와 응답 card ID가 다르면 구조 오류다. 스킨 제외 정책 밖의 모든 forward target이 최종 카드 집합에 존재하지 않으면 전체 빌드를 실패시킨다.

이 closure는 Standard 목록 카드와 재귀로 받은 `related` 카드에 적용한다. 뒤에서 추가하는 `class_reference` 카드가 참조하는 실제 게임 요소도 이어서 닫되, metadata의 `alternateHeroCardIds`에 명시된 영웅 스킨 ID는 pending 대상에서 제외한다.

`parentId`, `copyOfCardId` target은 추가 수집 원인이 아니다. ID 관계는 보존하지만 target row가 없을 수 있다. 실제 API의 `copyOfCardId`는 이름과 달리 배열일 수 있으므로 Raw 타입을 바꾸지 않고 정규화 relation 여러 행으로 옮긴다.

### 6.4 기본 영웅과 영웅 능력

`metadata.classes`의 `cardId`, `heroPowerCardId`가 최종 집합에 없으면 개별 카드 endpoint로 수집한다. `alternateHeroCardIds` 영웅 스킨은 첫 범위에서 수집하지 않는다. 기본 영웅 응답의 `childIds`에 같은 스킨 ID가 있어도 이를 따라가지 않으며 해당 공식 relation ID만 보존한다. 반면 강화 영웅 능력처럼 `alternateHeroCardIds`가 아닌 forward target은 실제 게임 요소이므로 related 카드로 수집한다.

### 6.5 획득 scope와 중복 우선순위

각 카드 row는 획득 원인 하나를 가진다.

```text
standard > class_reference > related
```

같은 ID가 여러 경로에서 발견되면 높은 우선순위가 이긴다. 기본 page 응답의 카드 객체를 다른 개별 응답으로 덮어쓰지 않는다.

### 6.6 locale 구조 동등성

두 locale은 다음 ID 집합이 정확히 같아야 한다.

- 기본 standard 카드 ID
- forward closure로 추가한 related 카드 ID
- class reference 카드 ID
- 정규화에 사용하는 taxonomy ID

localized name, text, flavor text, artist와 이미지 URL 문자열은 달라도 된다. 한국어 응답에 영문 fallback이 있거나 문자열이 비어 있어도 수집을 실패시키지 않는다. 한 locale의 값을 다른 locale에 복사하지 않는다.

## 7. HTTP와 retry

- connect timeout: 10초
- request 전체 timeout: 60초
- 최초 요청 뒤 최대 세 번 재시도한다.
- 재시도 대상: transport 오류, timeout, 408, 429, 500, 502, 503, 504
- `Retry-After`가 유효하면 이를 우선한다.
- 없으면 1초, 2초, 4초 지수 backoff에 jitter를 더한다.
- page와 개별 카드 요청은 순차 실행한다.
- 관찰된 Blizzard 제한인 시간당 client 36,000회와 초당 100회를 넘기는 병렬 수집을 도입하지 않는다.

한 locale이라도 retry를 소진하거나 검증에 실패하면 두 locale 패키지를 모두 발행하지 않는다. 다음 실행은 두 locale을 처음부터 다시 수집한다.

## 8. Raw JSON 계약

locale별 Raw는 canonical JSON 한 개다. HTTP bytes, 원래 whitespace와 원래 object key 순서를 보존할 필요는 없지만 JSON 값과 배열 의미는 보존한다.

최상위 구조는 다음 필드와 순서로 고정한다.

```json
{
  "format_version": 1,
  "source": {
    "provider": "blizzard",
    "api": "hearthstone_game_data",
    "region": "us",
    "endpoints": {
      "cards": "https://us.api.blizzard.com/hearthstone/cards",
      "card_by_id_template": "https://us.api.blizzard.com/hearthstone/cards/{card-id}",
      "metadata": "https://us.api.blizzard.com/hearthstone/metadata"
    }
  },
  "collected_at": "2026-08-05T00:00:00Z",
  "query": {
    "locale": "ko_KR",
    "set": "standard",
    "gameMode": "constructed",
    "collectible": "0,1",
    "pageSize": 500
  },
  "card_pages": [
    { "page": 1, "response": {} }
  ],
  "related_cards": [
    { "requested_card_id": 123, "response": {} }
  ],
  "class_reference_cards": [
    { "requested_card_id": 456, "response": {} }
  ],
  "metadata": {
    "response": {}
  }
}
```

`source.endpoints`는 query나 credential이 없는 고정 provenance다. 최상위 `query`는 카드 목록 요청의 page 제외 query다. `card_pages.page`를 합치면 목록 요청을 재구성할 수 있다. 개별 카드 요청은 같은 locale과 `requested_card_id`, metadata 요청은 같은 locale만 사용한다. 이는 모든 요청의 성공·실패 로그를 보존한다는 뜻이 아니다.

Blizzard response와 query 내부 필드 이름은 원래 camelCase를 유지한다. 앱 소유 wrapper만 snake_case다. Raw 안에 `data_version`, OAuth 정보, HTTP status, duration, credential이 포함될 수 있는 전체 URL/header와 retry 기록을 넣지 않는다. data version 결합은 manifest가 담당한다.

### 8.1 Raw 정렬과 canonical bytes

- UTF-8 compact JSON으로 쓰고 파일 끝에 LF 하나를 둔다.
- 앱 소유 object는 명세 필드 순서를 따른다.
- Blizzard response object key는 재귀적으로 사전순 정렬한다.
- Blizzard response 내부 배열 순서는 원본 그대로 둔다.
- `card_pages`는 page 오름차순이다.
- `related_cards`, `class_reference_cards`는 `requested_card_id` 오름차순이다.
- 카드 wrapper 중복은 scope 우선순위로 제거한다.

`collected_at`은 locale 수집을 시작할 때 주입한 UTC RFC 3339 값이다. 새 live 수집은 시각이 달라 다른 Raw bytes가 된다. 동일한 response, collected_at, serializer와 입력을 다시 패키징하면 동일한 bytes가 나와야 한다.

## 9. 정규화 원칙

locale마다 같은 schema의 독립 SQLite를 만든다.

```text
ko_KR.sqlite
en_US.sqlite
```

locale별 문자열을 한 DB나 한 table에 섞지 않는다. 빈 localized 문자열은 `NULL`로 바꾸되 다른 locale 값을 채우지 않는다. 공식 ID는 identity이며 같은 이름의 다른 ID를 합치지 않는다.

공식 응답의 현재 gameplay 필드는 타입 있는 컬럼이나 관계 행으로 정규화한다. 미래 미지 필드는 Raw에 즉시 보존하고 schema revision 전까지 SQLite에 억지로 넣지 않는다. Raw JSON 전체를 SQLite row에 중복 저장하지 않는다.

taxonomy 배열에 없지만 카드가 참조하는 ID는 해당 taxonomy table에 placeholder row를 만든다. placeholder는 ID만 가지며 localized `name`, `slug`와 부가 필드는 `NULL`이다.

## 10. SQLite schema

첫 schema는 13개 `STRICT` table을 사용한다.

```text
catalog_metadata
cards
sets
classes
card_types
rarities
minion_types
spell_schools
keywords
card_classes
card_minion_types
card_keywords
card_relations
```

`multiTypeIds`는 추가 card type이 아니라 추가 minion type ID다. 따라서 `card_minion_types`가 `minion_types`를 참조한다. `cards.type_id`는 `cardTypeId`만 보존한다.

### 10.1 DDL

```sql
CREATE TABLE catalog_metadata (
  singleton                  INTEGER PRIMARY KEY CHECK (singleton = 1),
  schema_version             INTEGER NOT NULL CHECK (schema_version > 0),
  data_version               TEXT NOT NULL,
  locale                     TEXT NOT NULL CHECK (locale IN ('ko_KR', 'en_US')),
  generated_at               TEXT NOT NULL,
  source_raw_sha256          TEXT NOT NULL CHECK (
    length(source_raw_sha256) = 64 AND
    source_raw_sha256 = lower(source_raw_sha256) AND
    source_raw_sha256 NOT GLOB '*[^0-9a-f]*'
  ),
  standard_card_count        INTEGER NOT NULL CHECK (standard_card_count >= 0),
  related_card_count         INTEGER NOT NULL CHECK (related_card_count >= 0),
  class_reference_card_count INTEGER NOT NULL CHECK (class_reference_card_count >= 0),
  total_card_count           INTEGER NOT NULL CHECK (
    total_card_count = standard_card_count + related_card_count + class_reference_card_count
  )
) STRICT;

CREATE TABLE sets (
  id   INTEGER PRIMARY KEY,
  slug TEXT,
  name TEXT
) STRICT;

CREATE TABLE card_types (
  id   INTEGER PRIMARY KEY,
  slug TEXT,
  name TEXT
) STRICT;

CREATE TABLE rarities (
  id                    INTEGER PRIMARY KEY,
  slug                  TEXT,
  name                  TEXT,
  crafting_cost_normal  INTEGER CHECK (crafting_cost_normal >= 0),
  crafting_cost_golden  INTEGER CHECK (crafting_cost_golden >= 0),
  dust_value_normal     INTEGER CHECK (dust_value_normal >= 0),
  dust_value_golden     INTEGER CHECK (dust_value_golden >= 0)
) STRICT;

CREATE TABLE minion_types (
  id   INTEGER PRIMARY KEY,
  slug TEXT,
  name TEXT
) STRICT;

CREATE TABLE spell_schools (
  id   INTEGER PRIMARY KEY,
  slug TEXT,
  name TEXT
) STRICT;

CREATE TABLE keywords (
  id       INTEGER PRIMARY KEY,
  slug     TEXT,
  name     TEXT,
  ref_text TEXT,
  text     TEXT
) STRICT;

CREATE TABLE cards (
  id                           INTEGER PRIMARY KEY,
  slug                         TEXT NOT NULL,
  scope_kind                   TEXT NOT NULL CHECK (
    scope_kind IN ('standard', 'class_reference', 'related')
  ),
  collectible                  INTEGER NOT NULL CHECK (collectible IN (0, 1)),
  name                         TEXT,
  text_markup                  TEXT,
  text_plain                   TEXT,
  flavor_text                  TEXT,
  artist_name                  TEXT,
  mana_cost                    INTEGER NOT NULL CHECK (mana_cost >= 0),
  attack                       INTEGER,
  health                       INTEGER,
  armor                        INTEGER,
  deck_size_mod                INTEGER,
  set_id                       INTEGER NOT NULL REFERENCES sets(id),
  type_id                      INTEGER NOT NULL REFERENCES card_types(id),
  rarity_id                    INTEGER REFERENCES rarities(id),
  spell_school_id              INTEGER REFERENCES spell_schools(id),
  image_url                    TEXT,
  crop_image_url               TEXT,
  rune_blood                   INTEGER CHECK (rune_blood >= 0),
  rune_frost                   INTEGER CHECK (rune_frost >= 0),
  rune_unholy                  INTEGER CHECK (rune_unholy >= 0),
  sideboard_max_cards          INTEGER CHECK (sideboard_max_cards >= 0),
  sideboard_subset             TEXT,
  sideboard_ignores_class      INTEGER CHECK (sideboard_ignores_class IN (0, 1)),
  sideboard_cards_count_as_max INTEGER CHECK (sideboard_cards_count_as_max IN (0, 1)),
  banned_from_sideboard        INTEGER NOT NULL CHECK (banned_from_sideboard IN (0, 1)),
  zilliax_functional_module    INTEGER NOT NULL CHECK (zilliax_functional_module IN (0, 1)),
  zilliax_cosmetic_module      INTEGER NOT NULL CHECK (zilliax_cosmetic_module IN (0, 1)),
  CHECK (
    (rune_blood IS NULL AND rune_frost IS NULL AND rune_unholy IS NULL) OR
    (rune_blood IS NOT NULL AND rune_frost IS NOT NULL AND rune_unholy IS NOT NULL)
  ),
  CHECK (
    (sideboard_max_cards IS NULL AND sideboard_subset IS NULL AND
     sideboard_ignores_class IS NULL AND sideboard_cards_count_as_max IS NULL) OR
    (sideboard_max_cards IS NOT NULL AND sideboard_subset IS NOT NULL AND
     sideboard_ignores_class IS NOT NULL AND sideboard_cards_count_as_max IS NOT NULL)
  )
) STRICT;

CREATE TABLE classes (
  id                         INTEGER PRIMARY KEY,
  slug                       TEXT,
  name                       TEXT,
  default_hero_card_id       INTEGER REFERENCES cards(id) DEFERRABLE INITIALLY DEFERRED,
  default_hero_power_card_id INTEGER REFERENCES cards(id) DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE card_classes (
  card_id     INTEGER NOT NULL REFERENCES cards(id),
  class_id    INTEGER NOT NULL REFERENCES classes(id),
  position    INTEGER NOT NULL CHECK (position >= 0),
  source_kind TEXT NOT NULL CHECK (source_kind IN ('primary', 'multi')),
  PRIMARY KEY (card_id, position),
  UNIQUE (card_id, class_id)
) STRICT;

CREATE TABLE card_minion_types (
  card_id        INTEGER NOT NULL REFERENCES cards(id),
  minion_type_id INTEGER NOT NULL REFERENCES minion_types(id),
  position       INTEGER NOT NULL CHECK (position >= 0),
  source_kind    TEXT NOT NULL CHECK (source_kind IN ('primary', 'multi')),
  PRIMARY KEY (card_id, position),
  UNIQUE (card_id, minion_type_id)
) STRICT;

CREATE TABLE card_keywords (
  card_id    INTEGER NOT NULL REFERENCES cards(id),
  keyword_id INTEGER NOT NULL REFERENCES keywords(id),
  position   INTEGER NOT NULL CHECK (position >= 0),
  PRIMARY KEY (card_id, position),
  UNIQUE (card_id, keyword_id)
) STRICT;

CREATE TABLE card_relations (
  source_card_id INTEGER NOT NULL REFERENCES cards(id),
  relation_kind  TEXT NOT NULL CHECK (
    relation_kind IN ('child', 'bundled', 'parent', 'copy_of')
  ),
  source_field   TEXT NOT NULL CHECK (
    source_field IN ('childIds', 'bundledCardIds', 'parentId', 'copyOfCardId')
  ),
  target_card_id INTEGER NOT NULL,
  display_order  INTEGER NOT NULL CHECK (display_order >= 0),
  PRIMARY KEY (source_card_id, relation_kind, display_order),
  UNIQUE (source_card_id, relation_kind, target_card_id),
  CHECK (
    (relation_kind = 'child'   AND source_field = 'childIds') OR
    (relation_kind = 'bundled' AND source_field = 'bundledCardIds') OR
    (relation_kind = 'parent'  AND source_field = 'parentId') OR
    (relation_kind = 'copy_of' AND source_field = 'copyOfCardId')
  )
) STRICT;

CREATE INDEX idx_cards_scope
  ON cards(scope_kind, id);
CREATE INDEX idx_card_classes_class
  ON card_classes(class_id, card_id);
CREATE INDEX idx_card_minion_types_type
  ON card_minion_types(minion_type_id, card_id);
CREATE INDEX idx_card_keywords_keyword
  ON card_keywords(keyword_id, card_id);
CREATE INDEX idx_card_relations_target
  ON card_relations(target_card_id, relation_kind, source_card_id);
```

공식 metadata에는 `craftingCost: [null, 400]`, `dustValue: [5, null]`처럼 normal/golden 중 한 값만 `null`인 pair가 실제로 존재한다. 네 currency column은 각각 독립 nullable 값으로 보존하며, 존재하는 값에는 개별 non-negative constraint만 적용한다.

`card_relations.target_card_id`에는 FK를 걸지 않는다. `child`, `bundled` target 존재는 build validation이 강제하되 class-reference source가 가리키는 `alternateHeroCardIds` 스킨은 예외다. 이 스킨 관계와 모든 `parent`, `copy_of` target은 현재 수집 범위 밖일 수 있기 때문이다.

`source_field`는 관계를 만든 공식 JSON key를 그대로 저장한다. 기존 query-pool mock의 `parent.childIds` 같은 값은 화면에서 방향을 설명하던 역사적 label이며, HCL-006 정규화 값은 공식 필드명인 `childIds`다.

### 10.2 텍스트 정규화

공식 `text`가 빈 문자열이면 `text_markup`, `text_plain`을 모두 `NULL`로 저장한다. 값이 있으면 `text_markup`은 공식 API 문자열을 그대로 보존하고 `text_plain`은 다음 고정 순서로 파생한다.

1. CRLF와 CR을 LF로 통일한다.
2. `<br>`, `<br/>`, `<br />`를 LF로 바꾼다.
3. 나머지 markup tag를 제거하되 표시되는 문자열은 보존한다.
4. HTML entity를 Unicode 문자로 decode한다.
5. 문자열 바깥쪽 공백만 제거한다.

철자, 문장부호와 locale 문구는 보정하지 않고 연속 내부 공백도 합치지 않는다. `text_markup`은 HCL-006에서 공식 문자열 보존 필드라는 뜻이며, 프론트 표시를 위한 allowlist sanitize나 rich-text segment 계약은 후속 IPC 명세에서 정한다.

### 10.3 배열 정규화

- `classId`가 있으면 `card_classes` position 0, source `primary`로 넣는다.
- `multiClassIds`는 원본 순서로 이어서 source `multi`로 넣는다.
- `minionTypeId`가 있으면 `card_minion_types` position 0, source `primary`로 넣는다.
- `multiTypeIds`는 추가 minion type으로 원본 순서대로 source `multi`로 넣는다.
- `keywordIds`는 원본 순서를 `position`으로 보존한다.
- relation 배열은 원본 순서를 `display_order`로 보존하고 scalar `parentId`는 0을 사용한다.

공식 배열 안의 중복 ID는 구조 오류로 처리한다.

### 10.4 카드 선택 객체와 flag

`runeCost`가 없으면 룬 컬럼 세 개가 모두 `NULL`이다. 객체가 있으면 `blood`, `frost`, `unholy` 세 키와 0 이상 정수가 모두 필요하다. 현재 공식 데이터에서는 죽음의 기사 카드에만 나타나지만 DB에 class ID allowlist를 하드코딩하지 않는다.

`sideboard`가 없으면 네 sideboard 컬럼이 모두 `NULL`이다. 객체가 있으면 현재 공식 키 네 개와 올바른 타입이 모두 필요하다. `sideboardSubset`은 공식 opaque string으로 저장하며 HCL-006이 의미를 해석하지 않는다.

`bannedFromSideboard`는 sideboard 제공 설정과 다른 카드 자체 flag이므로 `cards`에 독립 보존한다. 누락된 boolean flag는 false로 정규화한다.

`imageGold`는 Raw에만 보존한다. gold 이미지 URL과 gold asset 상태를 SQLite에 넣지 않는다.

2026-08-05 live Standard 응답에서는 `runeCost` 객체 65개와 `sideboard` 객체 2개를 확인했다. 이 개수는 fixture 관측값이며 validation 상수가 아니다.

### 10.5 SQLite 생성 설정

새 빈 파일에 다음 값을 table 생성 전에 명시한다.

```text
PRAGMA encoding = 'UTF-8'
PRAGMA page_size = 4096
PRAGMA auto_vacuum = NONE
PRAGMA journal_mode = DELETE
PRAGMA foreign_keys = ON
PRAGMA synchronous = FULL
PRAGMA user_version = 1
```

한 locale의 schema 생성과 전체 insert는 단일 transaction이다. taxonomy는 ID 오름차순, cards는 ID 오름차순, 연결·관계는 PK 오름차순으로 삽입한다. commit 뒤 `PRAGMA foreign_key_check`가 0행이고 `PRAGMA integrity_check`가 정확히 `ok`인지 검사한다.

DB는 고정 SQLite dependency와 같은 생성 순서에서 byte deterministic해야 한다. WAL, FTS, `ANALYZE`, 검색용 index와 `name_choseong` 컬럼은 만들지 않는다.

## 11. 출력과 원자성

성공 결과는 정확히 다음 다섯 파일이다.

```text
output/<data-version>/
├─ raw/
│  ├─ ko_KR.json.zst
│  └─ en_US.json.zst
├─ normalized/
│  ├─ ko_KR.sqlite.zst
│  └─ en_US.sqlite.zst
└─ manifest.json
```

pipeline은 output root 아래 data version과 충돌하지 않는 staging directory에서 전부 생성한다. 두 locale 수집, 정규화, 압축, hash와 manifest 검증이 모두 성공한 뒤에만 최종 version directory로 원자적 rename한다.

실패하면 staging directory를 자동 삭제한다. 최종 manifest나 부분 version directory를 남기지 않는다.

## 12. zstd 계약

Raw JSON과 SQLite 각각을 단일 zstd frame으로 압축한다.

- compression level: 10
- worker thread: 1
- content size: 기록
- frame checksum: 사용
- dictionary: 사용하지 않음
- dependency version: lockfile로 고정

같은 uncompressed bytes와 같은 compressor version·설정이면 compressed bytes가 같아야 한다.

## 13. manifest 계약

manifest는 기존 R2 설계의 camelCase를 유지하되 첫 구현에서 실제 생성한 네 asset만 기술한다.

`schemaVersion`과 `minimumAppVersion`은 `card-data-contract`의 versioned 상수다. 첫 값은 각각 `1`, `0.1.0`이다. 실행 인자로 임의 변경하지 않으며 schema 호환성이 바뀌는 코드 변경에서 함께 검토한다.

최상위 필드:

```text
schemaVersion: 1
minimumAppVersion: "0.1.0"
dataVersion: validated data version
officialPatchVersion: dataVersion에서 파싱한 string
buildId: dataVersion에서 파싱한 positive integer
revision: dataVersion에서 파싱한 positive integer
generatedAt: UTC RFC 3339
supportedLocales: ["ko_KR", "en_US"]
locales: locale key object
```

`generatedAt`은 두 SQLite의 `catalog_metadata.generated_at`과 같은 값이다. Raw의 실제 API 수집 시각은 각 Raw `collected_at`이 담당한다. asset별 timestamp는 두지 않는다.

각 locale object는 다음을 가진다.

```text
cardCounts.standard
cardCounts.related
cardCounts.classReference
cardCounts.total
raw
normalized
```

각 `raw`, `normalized` asset은 다음 필드를 가진다.

```text
path: version directory 기준 상대 경로
bytes: compressed file size
sha256: compressed file SHA-256 lowercase hex
compression: "zstd"
uncompressedBytes: uncompressed file size
uncompressedSha256: uncompressed file SHA-256 lowercase hex
defaultDownload: raw=false, normalized=true
```

`supportedLocales` 원소와 `locales` key 집합은 정확히 같아야 한다. 두 locale의 네 card count는 구조 동등성 검증 결과와 일치해야 한다. `total`은 나머지 세 scope count의 합이다.

manifest `schemaVersion`, SQLite `catalog_metadata.schema_version`, SQLite `PRAGMA user_version`은 모두 정확히 `1`이어야 한다. manifest와 SQLite의 모든 SHA-256 문자열은 정확히 64자의 lowercase hex인지 contract validator가 검사한다.

SQLite `catalog_metadata.source_raw_sha256`은 같은 locale Raw asset의 `uncompressedSha256`과 같아야 한다.

첫 manifest에는 `diff`, `imageMap`, `bootstrap`, `canonicalDeltas`, `imageIndex`, signature와 R2 pointer placeholder를 넣지 않는다.

## 14. JSONL 로그

stdout은 향후 machine output을 위해 비워 둔다. 실행 로그는 stderr에 JSON object 한 줄씩 쓴다.

모든 행의 필수 필드:

```text
schema_version
timestamp
level
stage
event
```

필요한 경우에만 다음을 추가한다.

```text
locale
attempt
status_code
counts
error_code
message
```

성공한 개별 HTTP 요청을 모두 기록하지 않는다. stage 시작·완료, retry, locale 요약과 최종 결과만 기록한다. 로그에 secret, token, Authorization header, token이 포함된 query·URL을 기록해서는 안 된다.

## 15. exit code

```text
0  성공
2  CLI, config, 자격증명 누락, 기존 output 충돌
3  OAuth와 token 인증 실패
4  network, timeout, retry 소진
5  API 구조, pagination, relation closure, locale 동등성 실패
6  정규화, SQLite schema·constraint·무결성 실패
7  압축, hash, manifest, output I/O와 원자적 완성 실패
```

## 16. 테스트 계약

### 16.1 기본 offline fixture

작은 canonical fixture 하나에 두 locale과 다음 구조를 포함한다.

- 일반 수집 카드와 비수집 관련 카드
- child, bundled, parent, copy relation
- 다중 직업
- 복수 minion type
- keyword 순서
- 룬 비용
- sideboard 설정과 sideboard 금지 flag
- 장소, 영웅, 영웅 능력
- 빈 localized 문자열
- 알려지지 않은 taxonomy placeholder
- markup tag, `<br>`와 HTML entity가 함께 있는 카드 text

실패 fixture를 통째로 복제하지 않는다. 테스트가 canonical fixture를 memory에서 변형해 다음 실패를 만든다.

- page 누락과 불연속
- `cardCount`, `pageCount` 불일치
- 카드 ID와 공식 배열 ID 중복
- 필수 metadata 배열 누락이나 타입 오류
- 수집 대상 child·bundled target 누락
- class-reference source의 미수집 스킨 target ID 보존
- 요청 card ID와 응답 ID 불일치
- locale별 카드·taxonomy ID 집합 불일치
- scalar, array, object 타입 오류
- 부분 `runeCost`, 부분 `sideboard`
- relation kind와 맞지 않는 `source_field`
- 잘못된 길이, 대문자 또는 비 hex 문자가 있는 SHA-256
- SQLite CHECK와 FK 위반
- 기존 data version output 충돌

미지 taxonomy ID는 실패가 아니라 placeholder 성공 사례다. 빈 번역, 영문 fallback과 빈 text도 실패가 아니다.

성공 fixture는 추가로 다음을 assertion한다.

- `text_markup`은 공식 문자열과 같고 `text_plain`은 고정 변환 결과와 같다.
- 각 relation의 `source_field`와 `display_order`가 공식 필드와 원본 배열 순서를 보존한다.
- Raw endpoint, 카드 목록 query, page와 개별 요청 ID로 성공 응답의 요청 provenance를 재구성할 수 있다.
- manifest `schemaVersion`, SQLite metadata schema version과 `PRAGMA user_version`이 모두 `1`이다.
- manifest `minimumAppVersion`이 `0.1.0`이다.

### 16.2 mock HTTP

- 429 `Retry-After` 뒤 성공
- retry 대상 5xx 뒤 성공
- timeout과 transport failure retry 소진
- 401 뒤 token refresh 한 번으로 성공
- refresh 뒤 다시 401
- OAuth HTTP 실패와 token field 누락
- secret과 token 로그 유출 검사

### 16.3 결정성과 실패 정리

고정 fixture, clock, data version으로 전체 pipeline을 두 번 실행한다. 다음 bytes가 각각 같아야 한다.

- Raw JSON
- SQLite
- Raw·SQLite zstd
- manifest

logical SQLite assertion도 별도로 실행한다. golden binary는 저장소에 commit하지 않는다.

압축, hash, manifest 쓰기와 rename 실패를 주입해 staging과 최종 manifest가 남지 않는지 검사한다. 성공 output에는 네 asset과 manifest만 있어야 한다.

### 16.4 live smoke

기본 test suite는 network가 없는 offline fixture다. release verification 전에는 같은 production build path를 쓰는 ignored test를 명시적으로 실행한다.

```powershell
cargo test -p card-data-pipeline --test live_smoke -- --ignored
```

live smoke는 OS temporary directory에 두 locale Raw, SQLite와 manifest를 끝까지 만들고 자체 검증한 뒤 data file을 삭제한다. 성공·실패 근거는 JSONL과 test result로 남긴다. live-only test나 smoke 전용 축약 수집기를 만들지 않는다.

## 17. 완료 조건

구현은 다음 조건을 모두 만족해야 한다.

- 승인된 고정 query로 두 locale current Standard 데이터를 수집한다.
- 영웅 스킨을 제외한 forward relation closure와 기본 class reference를 빠짐없이 보존한다.
- canonical Raw와 13-table locale SQLite를 만든다.
- 두 locale 구조 불일치나 부분 실패를 발행하지 않는다.
- 같은 고정 입력에 byte deterministic package를 만든다.
- secret 없는 JSONL과 정확한 exit code를 반환한다.
- offline fixture, fault injection, 결정성 test와 live smoke가 통과한다.
- first-slice output 외 이미지, diff, R2, updater 기능을 섞지 않는다.
