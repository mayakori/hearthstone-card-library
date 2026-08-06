# HCL-014 R2 Raw 후보 업로드 구현 계획

## 목표와 경계

HCL-006 production CLI를 GitHub Actions에서 수동 실행하고, 전체 패키지를 검증한 뒤 R2 candidate prefix에는 두 locale Raw zstd만 업로드한다. 예약 실행, production pointer, 서명, normalized R2 배포와 앱 소비는 범위 밖이다.

## 구현 단계

1. `tests/card-data-raw-r2-workflow.test.mjs`에 Raw-only receipt, 변조 거부, 재다운로드 검증과 workflow 금지사항을 RED로 고정한다.
2. `scripts/card-data-raw-r2-candidate.mjs`가 HCL-006 manifest와 실제 Raw bytes를 대조해 run-unique candidate receipt를 만들고 다운로드 결과를 검증하게 한다.
3. `.github/workflows/card-data-raw-r2-candidate.yml`에서 수동 입력, 최소 권한, production CLI 실행, 7일 Actions artifact, 두 Raw 객체 R2 업로드와 재다운로드 검증을 연결한다.
4. `README.md`에 필요한 GitHub Secrets·Variables와 비발행 경계를 기록한다.
5. 실제 HCL-006 live package로 receipt를 만들고 `npm run check`, `/va HCL-014`, `npm run merge:check -- HCL-014`를 실행한다.

## 완료 조건

- workflow trigger는 `workflow_dispatch` 하나다.
- R2 upload 입력은 검증된 `ko_KR`, `en_US` Raw zstd 두 파일뿐이다.
- 전체 package는 R2가 아니라 단기 GitHub Actions artifact에만 남는다.
- 업로드한 두 객체를 R2에서 다시 내려받아 manifest size와 SHA-256으로 검증한다.
- workflow와 helper는 Blizzard/R2 credential을 출력·파일화하지 않는다.
- `stable.json`, `versions.json`, `current` 계열 pointer를 만들거나 변경하지 않는다.
- 전체 저장소 검증과 merge gate가 통과한다.
