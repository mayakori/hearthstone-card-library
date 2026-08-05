# 공식 카드 데이터 배포 작업 핸드오프

아래 프롬프트를 `카드 수집 백엔드 테스트 확인` 작업에서 이어서 사용한다.

```text
작업 루트:
C:\Users\main\Desktop\Claude_Project\hearthstone-card-library

이 핸드오프는 `공식 카드 API 수집 방법 탐색` 작업에서 조사하고 합의한 공식 카드 데이터·이미지 배포 방향을 현재 데이터 수집 및 이미지 캐시 설계에 합치기 위한 것이다.

먼저 다음 파일을 읽는다.

- AGENTS.md
- README.md
- docs/TODO.md의 HCL-006과 HCL-011
- docs/design/card-data-contract-mock.md
- docs/design/card-data-contract-mock.ko-KR.json
- docs/handoff-card-data-release.md

사용자가 승인하기 전에는 런타임 코드, GitHub Actions, 추적 문서를 수정하지 않는다. 먼저 현재 설계와 아래 결정을 대조하고, 남은 선택을 한 번에 하나씩 확인한다.

## 결정된 사항

1. 카드 메타데이터의 정본은 Blizzard 공식 Hearthstone Game Data API로 한다.
2. HearthstoneJSON은 HearthSim이 게임 파일을 추출해 제공하는 비공식 제3자 서비스이므로 주 공급원으로 채택하지 않는다.
3. 배포자가 발급받은 Blizzard API 자격증명은 GitHub Actions 또는 다른 신뢰된 CI에서만 사용한다.
4. `client_secret`을 Tauri 바이너리, 프론트엔드, 공개 설정, GitHub Release Asset에 포함하지 않는다. 최종 사용자는 API 키를 발급받지 않는다.
5. 공식 API에서 수집·정규화한 카드 데이터와 공식 응답이 가리키는 이미지 파일을 앱 사용자용 데이터 묶음으로 패키징한다.
6. 대형 카드 JSON과 이미지 파일은 Git 커밋, Git LFS, Actions Artifact가 아니라 GitHub Release Assets로 배포한다. Actions Artifact는 검증 중간 산출물로만 사용한다.
7. 토이 프로젝트 단계에서는 별도 데이터 저장소를 만들지 않고 현재 `hearthstone-card-library` 저장소 하나를 사용한다.
8. 앱과 데이터의 Release 네임스페이스를 분리한다.
   - `vX.Y.Z`: Tauri 앱과 설치 파일
   - `data-YYYY.MM.DD.N`: 카드 데이터와 이미지
9. 데이터 갱신 때 같은 Asset을 덮어쓰지 않고 새 `data-*` Release를 만든다. CDN 캐시, 게시 중 파일 혼합, 롤백 문제를 피하기 위한 결정이다.
10. 데이터 Release는 앱의 `latest`를 가로채지 않게 `make_latest=false` 또는 동등한 정책을 사용한다.
11. 데이터 갱신기는 `/releases/latest`에 의존하지 않는다. GitHub Releases API에서 `data-*` 태그를 필터링해 최신 데이터 버전을 찾는다.
12. 로컬 데스크톱 앱은 GitHub webhook을 직접 받지 않는다. 앱 시작 시, 마지막 확인 후 24시간 경과 시, 사용자 수동 요청 시 최신 데이터 Release를 폴링한다.
13. Tauri 앱 updater와 카드 데이터 updater를 분리한다.
    - `AppUpdater`: Tauri SemVer와 서명 기반 설치 파일 업데이트
    - `CardDataUpdater`: Rust 백엔드의 카드 JSON·이미지 업데이트
14. `CardDataUpdater`는 임시 경로 다운로드, SHA-256 검증, 압축 및 스키마 검증, 성공 시 원자적 교체 순서로 동작한다. 실패하면 기존 데이터와 캐시를 유지한다.
15. 데이터 Release에는 최소한 `manifest.json`, 정규화 카드 데이터 압축 파일, 이미지 팩을 포함한다.
16. manifest에는 다음 정보를 둔다.
    - `schemaVersion`
    - `dataVersion`
    - `locale`
    - `fetchedAt`
    - `publishedAt`
    - 공식 source 또는 build 식별자
    - 각 파일의 이름 또는 URL, byte size, SHA-256
17. GitHub Actions는 저장소 Secret의 `BLIZZARD_CLIENT_ID`, `BLIZZARD_CLIENT_SECRET`을 사용한다.
18. 데이터 발행 Workflow는 `schedule`과 `workflow_dispatch`, 동시 발행 방지 concurrency, 최소 `contents: write`, 실패 시 미게시, 비밀값 로그 금지를 갖는다.
19. 공개 저장소 예약 Workflow는 60일간 저장소 활동이 없으면 자동으로 꺼질 수 있으므로 수동 실행 경로를 유지한다.
20. GitHub Release Asset 한도는 파일당 2 GiB 미만, Release당 최대 1,000개다. 이미지 팩은 개별 한도보다 충분히 작게 분할한다.

## 약관상 인지하고 수용한 리스크

- Blizzard API 약관은 등록된 앱을 통해 최종 사용자에게 개인 용도로 데이터를 배포하는 것을 허용한다.
- 동시에 API 데이터에 최대 30일 TTL과 최소 30일마다 갱신할 의무를 둔다.
- 따라서 영구 정적 스냅샷은 명시 조항과 충돌한다.
- 사용자는 앱이 무료 토이 프로젝트라는 전제에서 이 리스크를 감수하고 GitHub Release 패키징을 진행하기로 했다.
- 설계 문서에는 이를 단순히 회색지대라고 표현하지 말고 `인지하고 수용한 리스크`로 기록한다.
- 가능한 완화 조치는 앱 무료·비광고 유지, Blizzard 출처와 비제휴 고지, 30일 이내 자동 갱신, 오래된 데이터 Release 정리, API 키 비공개다.
- 공식 API가 제공한 URL에서 받은 이미지도 별도 예외가 없으므로 보수적으로 같은 API 데이터 범위로 취급한다.

## 이어서 결정할 사항

1. 이미지 팩 분할 단위: 전체 한 묶음, 카드 세트별, 카드 ID 해시 샤드 중 선택
2. 이미지 변형: normal, crop, gold 중 기본 포함과 선택 다운로드 범위
3. 데이터 Release 보존 정책: 현재와 직전만 유지할지, 30일 안의 복수 버전을 유지할지
4. 최초 설치: seed 데이터를 앱에 포함할지, 첫 실행 때 내려받을지
5. GitHub API 실패·오프라인·부분 다운로드 때의 사용자 경험
6. 데이터 Release와 `CardDataUpdater`를 HCL-006/HCL-011에 포함할지, 별도 HCL 작업으로 분리할지

현재 논의 중인 `로컬 이미지 캐시 이전`과 `GitHub Release의 배포 이미지 팩`은 서로 다른 개념이다.

- 로컬 이미지 캐시: 사용자 장치에서 다시 생성 가능한 캐시
- 배포 이미지 팩: 새 설치와 데이터 갱신의 공식 입력

두 개념을 섞지 말고 로컬 이미지 캐시 이전 여부를 다시 판단한다. 가장 먼저 이 구분을 사용자에게 설명하고, 로컬 이미지 캐시까지 이전할지 한 가지 질문만 제시하며 설계를 계속한다.

근거:

- Blizzard API 약관: https://www.blizzard.com/en-us/legal/a2989b50-5f16-43b1-abec-2ae17cc09dd6/blizzard-developer-api-terms-of-use
- GitHub Releases: https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases
- GitHub Releases API: https://docs.github.com/en/rest/releases/releases
- GitHub Actions 예약 이벤트: https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows
- Tauri updater: https://v2.tauri.app/ko/plugin/updater/

이 핸드오프를 만든 작업에서는 런타임 코드나 설정을 수정하지 않았다.
```
