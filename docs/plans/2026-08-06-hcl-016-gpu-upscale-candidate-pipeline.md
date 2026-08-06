# HCL-016 로컬 GPU 이미지 업스케일 후보 파이프라인 구현 계획

## 목표와 증거

HCL-015 R2 image candidate의 normal pack을 신뢰 경계로 검증하고 RTX 4090에서 Real-ESRGAN x4 → Lanczos x2 → 원본 alpha 복원을 수행한다. 파생 pack/map을 별도 R2 prefix에 업로드·재다운로드 검증하고 receipt를 마지막에 게시한다.

## 수정 경계

- 수정: `scripts/`, `tests/`, `.github/workflows/`, `requirements/`, HCL-016 spec/plan, `package.json`, `README.md`
- 제외: `src/`, `src-tauri/`, Rust schema, HCL-014/HCL-015 production behavior, 앱 pointer와 consumer
- main 전용 tracking 파일은 기능 worktree에서 수정하지 않는다.

## 1. RED — 입력과 workflow 계약

- source/derived prefix, receipt object set, normal-only selection, locale별 deterministic sample과 unsafe path 거부 테스트
- manual-only trigger, exact self-hosted labels, read-only permission, timeout/concurrency, Blizzard secret 부재와 receipt-last 순서 테스트
- 실행: `python -m unittest tests/python/test_card_image_upscale.py`, `npm run workflow:test`

## 2. GREEN — source 획득과 검증

- Python CLI가 R2 source receipt와 maps를 먼저 다운로드하고 normal 참조가 요구하는 pack만 다운로드한다.
- receipt size/SHA와 tar.zst member path/hash를 검증하고 locale별 source selection을 만든다. pack 이름으로 variant를 추론하지 않는다.
- R2/Boto3 import는 network command 경계에서만 요구해 fixture tests는 표준 라이브러리만으로 실행한다.

## 3. GREEN — GPU 변환과 산출물

- 공식 tool archive와 executable/model hashes를 검증한다.
- RGBA PNG inference input, GPU batch 실행, x2/alpha 후처리, output decode/dimension/hash 검증을 구현한다.
- deterministic map/tar.zst와 canonical receipt를 생성하고 local verifier를 다시 통과시킨다.

## 4. GREEN — R2 receipt-last workflow

- pack/map upload → 별도 download root → full verify → receipt upload/get exact compare 순서를 구현한다.
- metadata artifact와 step summary에 source/derived prefix, mode/count와 검증 상태를 남긴다.
- 실패/성공 cleanup에서 x4 intermediate를 삭제한다.

## 5. 검증과 live smoke

1. fixture/unit/workflow tests
2. `npm run check`
3. 최신 main 반영 후 동일 검증
4. `/va HCL-016`, `npm run merge:check -- HCL-016`
5. squash merge와 승인된 push
6. `source_candidate_prefix=candidates/images/36.0.3-build247416-r1/runs/31066060504-1`, `max_images=10` 수동 실행
7. R2 derived receipt와 20개 output을 재검증하고 tracking 정본에 증거 기록
8. smoke 승인 후 `max_images=0` 전체 실행
