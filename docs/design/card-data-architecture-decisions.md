# 카드 데이터 아키텍처 결정 기록

Status: Approved decision record

Approved: 2026-07-25 KST

Updated: 2026-07-25 KST
Scope: 카드 수집, 정규화, 버전 스냅샷, 이미지, 쿼리·카드풀, 배포와 앱의 책임 경계

## 1. 문서의 역할

이 문서는 카드 데이터 관련 대화에서 확정된 결정을 한곳에 모은 정본 색인이다. 세부 스키마와 배포 계약을 다시 정의하지 않고 기존 설계의 우선순위와 관계를 명확히 한다.

세부 정본은 다음 문서다.

- [`card-data-r2-release-design.md`](./card-data-r2-release-design.md): R2 배포, 버전, 스냅샷, 이미지와 앱 갱신 계약
- [`card-data-contract-mock.md`](./card-data-contract-mock.md): 카드 원본·정규화·IPC 계약의 실데이터 기반 목업
- [`card-data-contract-mock.ko-KR.json`](./card-data-contract-mock.ko-KR.json): 기계 판독 가능한 카드 계약 샘플
- [`query-pool-schema-explorer.html`](./query-pool-schema-explorer.html): Query AST, 앱 기본·개인 카드풀과 검증 동작 목업

`docs/handoff-card-data-release.md`의 GitHub Release 기반 카드 데이터 배포안은 R2 설계로 대체됐다. Tauri 앱 바이너리는 계속 GitHub Releases로 배포하지만 카드 데이터와 이미지는 Cloudflare R2에서 배포한다.

## 2. 가장 중요한 책임 경계

패키징된 Tauri 앱은 Blizzard API를 직접 호출하거나 공식 원본을 정규화하지 않는다.

```text
Blizzard Game Data API + 공식 패치노트
                    │
                    ▼
       GitHub Actions의 Rust 데이터 파이프라인
       ├─ OAuth 인증과 전체 데이터 수집
       ├─ locale별 Raw 스냅샷 보존
       ├─ 정규화와 완성된 SQLite 생성
       ├─ diff와 이미지 변경 후보 계산
       ├─ 패키징·해시·서명
       └─ R2 발행
                    │
                    ▼
              Cloudflare R2
       ├─ 제어 문서와 버전 manifest
       ├─ Raw·정제·diff 데이터
       └─ normal·crop 이미지 팩
                    │
                    ▼
              패키징된 Tauri 앱
       ├─ 정제 SQLite와 이미지 팩 다운로드
       ├─ 서명·해시·스키마 검증
       ├─ 로컬 활성 버전 원자적 교체
       └─ SolidJS에 목록·상세·검색 제공
```

사용자 앱에는 Blizzard Client Secret, R2 쓰기 자격증명과 데이터 정제 코드가 포함되지 않는다. 앱은 공개 R2 객체를 읽기만 한다.

별도 상시 서버는 운영하지 않는다. 신뢰된 GitHub Actions가 배포자 역할을 하고 R2는 정적 배포 원본 역할을 한다.

## 3. 저장소와 실행 단위

초기에는 앱과 데이터 파이프라인을 별도 저장소로 나누지 않는다. 같은 저장소의 Rust workspace 안에서 책임을 분리한다.

```text
hearthstone-card-library/
├─ crates/
│  ├─ card-data-contract/     # Raw·정규화·manifest 공용 타입
│  └─ card-data-pipeline/     # 수집·정제·패키징 CLI
├─ src-tauri/                 # 사용자 앱 백엔드
├─ src/                       # SolidJS 프론트엔드
├─ data/fixtures/             # 공식 응답과 파이프라인 테스트 자료
└─ .github/workflows/         # 수집·검증·R2 발행 자동화
```

`card-data-pipeline`은 로컬 개발과 GitHub Actions에서 실행하지만 Tauri 앱 설치 파일에는 포함하지 않는다. `card-data-contract`는 파이프라인과 앱이 같은 필드 이름과 버전 규칙을 사용하도록 한다.

파이프라인이 독립적인 릴리스 주기나 권한 경계를 요구할 만큼 커질 때만 별도 저장소 분리를 다시 검토한다.

## 4. 공식 데이터 원본

- 카드 메타데이터의 정본은 Blizzard 공식 Hearthstone Game Data API다.
- 기본 지원 locale은 `ko_KR`, `en_US`다.
- 공식 API는 과거 카드 상태와 패치노트를 제공하지 않으므로 앱의 과거 열람은 자체 스냅샷으로 보존한다.
- 공식 패치 번호와 발표된 변경은 Hearthstone 공식 패치노트 어댑터가 별도로 수집한다.
- 패치노트의 발표 내용과 실제 API diff는 서로 덮어쓰지 않고 별도 사실로 저장한다.
- 홈페이지 카드 라이브러리의 내부 `/api/cards`와 HTML 내 메타데이터를 직접 가져오는 방식은 탐색용 실험이었으며 운영 수집 경로에서는 사용하지 않는다.

지원 locale의 카드 JSON과 메타데이터는 패치 확인 시 전체 수집한다. 이미지는 JSON diff로 후보를 좁힌 뒤 필요한 것만 확인한다.

## 5. Raw 데이터

Raw와 정제 데이터는 모두 저장하되 용도를 분리한다.

Raw 데이터는 다음 원칙을 따른다.

- 공식 API 응답의 의미를 바꾸지 않고 locale별로 보존한다.
- endpoint, 요청 파라미터, locale, 수집 시각과 응답 식별 정보를 함께 저장한다.
- 공식 응답의 빈 문자열, 잘못된 slug와 불완전한 텍스트를 임의로 고치지 않는다.
- 이미지 바이트를 JSON 안에 중복 저장하지 않고 공식 이미지 URL만 보존한다.
- 정규화 버그 수정, 재정제와 당시 응답 검증에 사용한다.
- 일반 사용자 앱의 기본 다운로드 대상이 아니다.

Raw 보존 형식은 압축된 JSON asset이며 버전 경로의 파일은 덮어쓰지 않는다.

## 6. 정규화 데이터

앱은 정제된 locale별 SQLite 스냅샷을 사용한다.

정규화 계층은 다음을 담당한다.

- 카드 ID를 기준으로 세트, 직업, 타입, 희귀도, 종족과 키워드를 조인한다.
- 이름, 효과 텍스트, 플레이버 텍스트, 아티스트와 수집 가능 여부를 보존한다.
- 제작 비용과 추출 가루를 일반·황금 값으로 구조화한다.
- 검색용 평문과 표시용 마크업을 분리한다.
- 빈 문자열은 의미상 값이 없는 경우 `null`로 바꾼다.
- 다중 값 필드인 키워드와 관계 ID는 값이 없어도 `null`이 아니라 빈 배열을 사용한다.
- 공식 텍스트의 보정이나 카드 효과 의미 추론은 원문을 덮어쓰지 않고 별도 필드와 provenance를 가진다.

각 데이터 버전은 완성된 정제 SQLite 스냅샷을 가진다. 과거 버전을 열 때 최초 버전부터 diff를 재생하지 않는다. diff는 변경 설명과 검증에만 사용한다.

## 7. 카드 관계와 비수집 카드

- 공식 관련 카드는 일반 카드와 같은 정규화 카드 모델에 저장한다.
- `collectible=false`인 생성 카드, 선택지와 토큰도 별도 공식 ID를 가진 카드 레코드로 보존한다.
- 같은 표시 이름과 능력치를 가져도 공식 ID나 이미지가 다르면 합치지 않는다.
- 부모의 `childIds`와 자식의 `parentId`를 모두 원본 필드로 보존한다.
- 자식의 `parentId`가 비어 있어도 부모의 `childIds`에 있으면 공식 관계를 구성한다.
- 카드 상세에서는 공식 관계 ID뿐 아니라 표시 가능한 관련 카드 요약을 함께 조립한다.

공식 카드 관계와 효과 기반 카드풀 연결은 다른 데이터다. 공식 관계는 카드 데이터에, 카드풀 정의와 노출 연결은 쿼리 데이터에 저장한다.

## 8. 검색 Query AST

검색 엔진과 저장 포맷은 중첩 가능한 Query AST 하나로 통일한다.

```text
기본 필터 UI ─┐
고급 트리 UI ─┼─→ Query AST ─→ 검색 엔진
관련 카드풀 ──┤
저장된 검색 ──┘
```

- 일반 필터 UI는 평면적인 AND·OR·NOT 입력을 받아 AST로 변환한다.
- 고급 사용자는 중첩된 AND·OR·NOT 트리를 직접 편집할 수 있다.
- 다중 값 필드는 `contains_any`, `contains_all`, `contains_none`을 지원한다.
- 직접 포함·제외할 카드 ID도 별도 우회 컬럼이 아니라 같은 AST 안의 `card_id` 조건과 NOT 그룹으로 표현한다.
- 필드와 연산자가 유효한 쿼리의 검색 결과가 0장인 것은 정상 성공이다.
- 제거된 의미 태그, 지원하지 않는 필드나 깨진 연산자는 실행 전 검증 오류로 차단하며 0건 결과로 위장하지 않는다.
- cursor는 외부에서 해석하지 않는 불투명 문자열이다.

기본 UI, 고급 UI와 카드풀은 입력 방식만 다르고 실행 엔진은 같은 AST만 이해한다.

## 9. 앱 기본 카드풀과 개인 카드풀

앱 기본 카드풀과 사용자가 만든 개인 카드풀을 모두 지원한다.

- 카드풀은 고정 카드 목록이 아니라 이름, 설명, origin과 Query AST를 가진 정의다.
- `origin=system`은 앱 기본 카드풀, `origin=user`는 개인 카드풀을 의미한다.
- 한 카드풀을 여러 기준 카드에 연결할 수 있으며 정의를 복제하지 않는다.
- 카드와 카드풀의 연결 메타데이터는 `card_query_pool_links`와 같은 별도 연결 구조에 둔다.
- 기준 카드의 공식 관계 메타데이터를 카드풀 테이블에 복제하지 않는다.
- 카드 상세에서 카드풀을 누르면 기존 라이브러리 검색 상태를 세션 기록에 보관하고 카드풀 AST를 적용한다.
- 카드 상세 화면에서는 카드풀의 예상 결과 장수를 미리 계산해 표시하지 않는다. 라이브러리에서 실제 실행한 뒤 결과를 표시한다.

카드 효과 의미 태그로 만든 카드풀은 공식 관련 카드와 구분한다. 의미 규칙이 유효하지 않으면 해당 카드풀 실행을 차단하고 수정이 필요한 조건 경로를 제공한다.

## 10. 데이터 버전과 패치 이력

데이터 버전은 다음 형식을 사용한다.

```text
36.0.3-build247416-r1
```

- `36.0.3`: 공식 패치 번호
- `build247416`: 확인된 공식 build 식별자
- `r1`: 같은 공식 상태를 다시 정제·배포한 내부 revision

공식 패치 번호가 같아도 build가 다르면 별도 데이터 버전이다. 정규화 오류를 수정해 다시 발행하면 기존 경로를 덮어쓰지 않고 revision을 올린다.

R2의 제어 문서는 다음 세 층으로 고정한다.

```text
channels/stable.json
catalog/versions.json
releases/<data-version>/manifest.json
```

- `stable.json`은 신규 설치의 현재 기본 버전을 가리킨다.
- `versions.json`은 사용자가 열람할 수 있는 패치 이벤트와 데이터 버전 목록이다.
- 버전별 manifest는 locale별 Raw, 정제본, diff, 이미지 맵과 해시를 고정한다.

카드 변화가 없는 공식 패치도 versions에 기록하지만 완성된 데이터와 이미지 바이트를 복제하지 않고 직전 스냅샷을 참조한다.

## 11. 이미지

- 지원 locale마다 `normal`, `crop` 이미지를 저장한다.
- `gold` 이미지는 수집·배포 대상에서 제외한다.
- 이미지 URL이 아니라 실제 바이트의 SHA-256을 정본 식별자로 사용한다.
- `cardId + locale + variant`별 이미지 해시를 버전 이미지 맵에 기록한다.
- 동일한 바이트는 URL이 달라도 한 번만 정본으로 저장한다.
- 바이트가 바뀌면 URL이 같아도 새 해시로 저장한다.

이미지 정본은 최초 기준팩과 패치별 변경팩으로 구성한다. 변경팩에는 이전 전역 이미지 인덱스에 없던 새 해시만 넣는다.

신규 설치 편의를 위해 현재 버전의 전체 이미지 설치팩을 별도로 제공한다.

- 신규 설치: 최신 전체 설치팩 사용
- 기존 설치 업데이트: 새 패치의 delta만 사용
- 과거 버전 열람: 해당 버전이 고정한 이미지 맵과 전역 정본 인덱스 사용

정본 압축팩이 480 MiB를 넘을 때만 결정론적으로 여러 archive로 분할한다. 이미지 한 장 자체를 조각내지 않는다.

## 12. R2 배포와 무결성

- 개발 중에는 현재 `r2.dev` 공개 주소를 사용한다.
- 공개 배포 전 `data.<project-domain>` 형태의 커스텀 도메인으로 교체한다.
- 앱은 하나의 `DATA_BASE_URL`만 알고 manifest의 상대 경로를 조합한다.
- R2 S3 endpoint와 쓰기 자격증명은 GitHub Actions Secret에서만 사용한다.
- 별도 Cloudflare Worker나 애플리케이션 서버는 초기 범위에 두지 않는다.

CI는 `stable.json`, `versions.json`과 버전 manifest를 Ed25519로 서명한다. 앱에는 공개 검증 키만 포함한다.

발행은 pointer-last 순서를 따른다.

```text
immutable asset 업로드
→ 해시·크기·압축 내용 재검증
→ manifest와 서명 업로드
→ versions 갱신
→ stable을 마지막에 갱신
```

stable 갱신 전 실패하면 사용자는 기존 완성 버전을 계속 사용한다.

## 13. 앱의 데이터 갱신

카드 데이터 갱신과 Tauri 앱 바이너리 갱신은 별도 기능이다.

카드 데이터 갱신은 `CardDataUpdater`가 담당한다.

- signed stable과 manifest를 검증한다.
- 정제 SQLite와 필요한 이미지 asset을 임시 경로에 받는다.
- 압축 파일과 내부 파일의 해시, 크기, 파일 수와 경로를 검증한다.
- 모든 검증을 통과한 뒤에만 활성 버전을 원자적으로 교체한다.
- 실패하면 기존 데이터로 계속 실행하고 재시도를 제공한다.
- 정확한 이미지 해시가 없으면 다른 버전의 이미지를 대신 표시하지 않고 플레이스홀더를 사용한다.

첫 실행은 기본 locale인 `ko_KR` 정제 DB를 먼저 받아 검색을 사용할 수 있게 한다. 이미지 설치팩은 crop을 우선하고 normal을 백그라운드에서 받는다. 다른 locale은 사용자가 처음 전환할 때 받는다.

앱 바이너리 갱신은 GitHub Releases와 Tauri updater의 한 번 클릭 업데이트로 별도 처리한다.

## 14. 로컬 캐시와 R2 정본

두 저장소는 다른 개념이다.

```text
Cloudflare R2
└─ 새 설치, 데이터 업데이트와 과거 버전 복구에 필요한 배포 원본

사용자 장치 로컬 캐시
└─ R2 원본으로 다시 생성할 수 있는 개별 이미지와 활성 DB
```

R2는 여러 파일을 담은 `tar.zst`를 사용할 수 있고 로컬은 content hash별 개별 파일을 사용한다. 설치형과 포터블 모드 사이의 설정·DB·캐시 이전 정책은 R2 보존 정책과 별도로 관리한다.

## 15. 첫 구현 명세의 경계

첫 구현 대상은 `card-data-pipeline`의 메타데이터 패키징 수직 절편이다.

```text
Blizzard OAuth
→ ko_KR·en_US 전체 카드와 메타데이터 수집
→ locale별 Raw 스냅샷
→ 정규화
→ locale별 완성 SQLite
→ zstd 압축
→ SHA-256과 manifest
→ 로컬 출력 폴더
```

첫 구현은 다음을 포함하지 않는다.

- 이미지 다운로드와 이미지 팩
- 패치노트 파싱과 diff
- R2 업로드와 전자서명
- GitHub Actions 예약·수동 워크플로
- Tauri `CardDataUpdater`와 IPC

로컬에서 생성하는 출력 형태는 다음을 기준으로 한다.

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

후속 구현 순서는 이미지 패키징, diff·패치 이력, R2 서명·발행, GitHub Actions 자동화, Tauri 데이터 갱신기다.

## 16. 아직 구현 명세에서 구체화할 항목

다음은 기존 제품 결정을 뒤집는 선택지가 아니라 구현 계획을 위해 정확한 타입과 동작을 적어야 하는 항목이다.

- Rust workspace와 crate별 실제 파일 배치
- 공식 OAuth token의 메모리 캐시와 만료 처리
- 카드·메타데이터 endpoint별 페이지네이션
- HTTP timeout, 재시도 대상 상태 코드와 backoff 상한
- Raw envelope의 정확한 필드와 안정적인 직렬화 순서
- 정규화 SQLite DDL, 인덱스와 transaction 경계
- 결정론적 zstd 설정과 manifest asset 필드
- 오류 종류, CLI exit code와 로그에서 비밀값을 숨기는 규칙
- fixture별 실패·성공 계약과 재현 가능한 패키지 검증

`text_markup`의 최종 표현, 검색 cursor 인코딩과 Tauri asset URL 변환은 첫 데이터 패키징 구현의 결과를 바꾸지 않으므로 후속 앱·IPC 명세에서 결정한다.
