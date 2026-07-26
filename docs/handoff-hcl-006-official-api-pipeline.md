# HCL-006 공식 API 수집 파이프라인 명세 핸드오프

Status: Ready for a separate Superpowers design session

Updated: 2026-07-25 KST

## 새 세션의 목표

Blizzard 공식 Hearthstone Game Data API에서 카드와 메타데이터를 수집해 locale별 Raw 스냅샷과 정규화 SQLite 패키지를 만드는 Rust CLI의 구현 명세를 작성한다.

제품 방향과 기존 데이터 스키마를 다시 설계하지 않는다. `superpowers:brainstorming`으로 실제 미결정 수집 계약만 구체화하고, 사용자 승인 후 명세서를 작성·커밋한다. 이후에만 `superpowers:writing-plans`로 구현 계획을 만든다.

GStack은 사용하지 않는다. 런타임 코드와 crate 구현은 명세 승인 전까지 시작하지 않는다.

## 먼저 읽을 파일

1. `AGENTS.md`
2. `docs/design/card-data-architecture-decisions.md`
3. `docs/design/card-data-r2-release-design.md`
4. `docs/design/card-data-contract-mock.md`
5. `docs/TODO.md`
6. 이 문서

실제 응답 구조는 다음 fixture에서 확인한다.

- `data/fixtures/r2-smoke/2026-07-25/raw/cards.ko_KR.json`
- `data/fixtures/r2-smoke/2026-07-25/raw/cards.en_US.json`
- `data/fixtures/r2-smoke/2026-07-25/normalized/cards.ko_KR.json`
- `data/fixtures/r2-smoke/2026-07-25/normalized/cards.en_US.json`
- `data/fixtures/r2-smoke/2026-07-25/normalized/metadata.ko_KR.json`
- `data/fixtures/r2-smoke/2026-07-25/normalized/metadata.en_US.json`
- `data/fixtures/r2-smoke/2026-07-25/manifest.json`

## 승인된 책임 경계

```text
Blizzard API와 공식 패치노트
→ GitHub Actions의 Rust 데이터 파이프라인
→ Cloudflare R2
→ 패키징된 Tauri 앱
```

- 패키징된 Tauri 앱은 Blizzard API를 호출하거나 공식 응답을 정규화하지 않는다.
- GitHub Actions가 신뢰된 수집·정제·발행 환경이다.
- 별도 상시 서버는 운영하지 않는다.
- 앱은 R2에서 검증된 정제 SQLite와 이미지 팩만 받는다.
- Blizzard API와 R2 쓰기 자격증명을 앱에 포함하지 않는다.
- 데이터 파이프라인과 앱은 같은 저장소의 Rust workspace에서 관리한다.

```text
crates/
├─ card-data-contract/
└─ card-data-pipeline/
```

`card-data-pipeline`은 로컬 개발과 GitHub Actions에서 실행하지만 Tauri 설치 파일에는 포함하지 않는다.

## 이미 결정된 데이터 정책

- 공식 Hearthstone Game Data API가 카드 데이터의 정본이다.
- 공식 홈페이지 내부 `/api/cards`와 HTML 메타데이터 수집 방식은 운영 경로에서 폐기했다.
- 공식 패치노트는 API diff와 별도 사실로 수집한다.
- 초기 locale은 `ko_KR`, `en_US`다.
- Raw와 정제 데이터를 모두 저장한다.
- 앱은 locale별 완성된 정규화 SQLite 스냅샷을 사용한다.
- 과거 버전을 열 때 diff 체인을 재생하지 않는다.
- `collectible=false`인 토큰과 선택지도 공식 ID를 가진 일반 카드 레코드로 보존한다.
- 같은 이름이라도 공식 ID가 다르면 합치지 않는다.
- 이미지 variant는 `normal`, `crop`만 수집하고 `gold`는 제외한다.
- 카드 데이터와 이미지는 R2, 앱 바이너리는 GitHub Releases로 배포한다.
- 데이터 버전은 `<official-patch>-build<build-id>-r<revision>` 형식이다.
- Query AST와 앱 기본·개인 카드풀은 이미 별도 설계가 승인됐으며 이번 명세에서 재논의하지 않는다.

## 첫 구현 명세의 범위

```text
Blizzard OAuth
→ ko_KR·en_US 카드와 메타데이터 수집
→ locale별 Raw 스냅샷
→ 정규화
→ locale별 완성 SQLite
→ zstd 압축
→ SHA-256과 manifest
→ 로컬 출력 폴더
```

기준 출력은 다음과 같다.

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

첫 명세에서 제외한다.

- 이미지 다운로드와 이미지 팩
- 패치노트 파싱과 diff
- R2 업로드와 전자서명
- GitHub Actions workflow 구현
- Tauri `CardDataUpdater`
- 프론트 IPC와 검색
- Query AST 재설계

## 로컬 자격증명 규칙

`.env.card-data.local`은 Git에서 무시된다. 현재 사용하는 변수명은 다음과 같다.

- `BLIZZARD_CLIENT_ID`
- `BLIZZARD_CLIENT_SECRET`

값, 발급 정보와 OAuth access token을 출력·로그·문서·fixture에 남기지 않는다. 읽기 전용 API 검증이 필요할 때 프로세스 메모리에서만 사용한다.

## 2026-07-25 실제 API 관측

이 절의 숫자는 명세 상수가 아니라 당시 응답을 확인한 관측값이다.

OAuth:

- endpoint: `https://oauth.battle.net/token`
- client credentials flow
- token type: `bearer`
- 관측된 `expires_in`: 86,399초

카드 목록:

- endpoint: `https://us.api.blizzard.com/hearthstone/cards`
- 최상위 필드: `cards`, `cardCount`, `pageCount`, `page`
- 기본/constructed: 6,347장
- Battlegrounds: 962장
- Mercenaries: 122장

메타데이터:

- endpoint: `https://us.api.blizzard.com/hearthstone/metadata`
- 주요 필드: `sets`, `setGroups`, `gameModes`, `types`, `rarities`, `classes`, `minionTypes`, `spellSchools`, `keywords`, `filterableFields`, `numericFields`, `cardBackCategories`
- `ko_KR` 관측 개수: sets 44, classes 12, types 9, rarities 5, minionTypes 25, spellSchools 9, keywords 74

## 다음 세션에서 이어갈 첫 질문

v1 수집 game mode 범위를 한 번에 하나의 질문으로 확인한다.

1. 전통 Hearthstone 카드 전체 — 권장
   - 수집 카드와 관련 토큰·선택지 등 비수집 카드 포함
   - Battlegrounds와 Mercenaries 제외
2. 전통 카드와 Battlegrounds
3. 전통 카드·Battlegrounds·Mercenaries 전체

사용자 답을 받기 전에 후속 질문을 한꺼번에 제시하지 않는다.

## 이후 구체화할 계약

- 전체 목록 페이지네이션과 page size
- OAuth token 메모리 캐시와 갱신
- HTTP timeout, 재시도 상태와 backoff
- locale 하나의 실패가 전체 패키지를 막는지
- Raw envelope의 정확한 필드
- 정규화 필수·선택 필드와 미지 taxonomy 처리
- SQLite DDL, 인덱스와 transaction
- 결정론적 zstd와 manifest
- 부분 실패 시 임시 출력 정리와 원자적 완성
- CLI exit code와 비밀값 마스킹
- fixture와 재현 가능한 패키지 테스트

## 명세 산출물

승인된 설계는 다음 경로에 작성한다.

`docs/superpowers/specs/2026-07-25-hcl-006-official-card-data-pipeline-design.md`

명세를 작성한 뒤 placeholder, 모순, 범위와 모호성을 자체 검토하고 사용자에게 최종 검토를 요청한다. 사용자 승인 전에는 구현 계획과 코드를 작성하지 않는다.

## 작업 상태

- HCL-006: `in_progress`
- 결정 정본과 추적 갱신 커밋: `e55335f`
- TODO·칸반 동기화 검사: 통과
- 프로덕션 수집·패키징 코드: 시작하지 않음
- 기존 dirty/untracked 파일: HCL-006 명세와 무관하면 수정·스테이징·커밋하지 않음
