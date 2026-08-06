# HCL-016 로컬 GPU 이미지 업스케일 후보 파이프라인 설계

## 1. 목표

HCL-015가 R2에 게시한 검증 완료 이미지 candidate를 입력 정본으로 삼아 `normal` 카드 이미지만 로컬 RTX 4090 self-hosted runner에서 업스케일한다. Real-ESRGAN x4 추론 결과를 Lanczos로 원본의 정확히 x2 크기로 축소하고, 원본 알파를 x2로 확대한 값으로 복원한다. 결과 pack, 파생 map과 receipt를 원본과 분리된 불변 R2 candidate 경로에 올리고 원격 바이트를 검증한 뒤 receipt를 마지막에 게시한다.

이 단계는 파생 이미지 candidate 생성까지다. 앱용 pointer, 정식 릴리스, HCL-015 원본 변경과 Tauri 소비 코드는 포함하지 않는다.

## 2. 승인된 경계

- 공개 저장소의 repository-scoped Windows x64 runner `hcl-rtx4090`을 사용한다.
- workflow trigger는 소유자만 실행할 수 있는 `workflow_dispatch` 하나다.
- runner label은 `self-hosted`, `Windows`, `X64`, `gpu`, `rtx4090`을 모두 요구한다.
- pull request, `pull_request_target`, push, schedule과 외부 입력 checkout은 금지한다.
- workflow 권한은 `contents: read`다.
- R2의 기존 네 설정만 사용하고 Blizzard 자격증명은 GPU job에 전달하지 않는다.
- workflow는 기존 HCL-015 candidate의 `receipt.json`과 두 locale map을 먼저 다운로드한다. map의 `normal` available 항목이 참조하는 pack만 다운로드한다. HCL-015의 전역 중복 제거로 normal 항목이 이름상 crop owner pack을 참조할 수 있으므로 pack 이름으로 입력 범위를 추정하지 않는다. Raw JSON에서 URL을 다시 읽거나 Blizzard CDN을 재호출하지 않는다.
- `crop`, `gold`, 영웅 스킨 보충, 자체 대체 이미지와 미존재 자산 생성은 제외한다.

## 3. 입력 계약

수동 입력은 다음 두 개다.

- `source_candidate_prefix`: `candidates/images/<data-version>/runs/<run-id>-<attempt>` 형식의 완료된 HCL-015 prefix
- `max_images`: locale별 최대 처리 수. `10`이면 20장 smoke, `0`이면 전체 처리

workflow는 source receipt를 먼저 내려받고 다음을 검증한다.

1. schema version 1과 입력 prefix exact match
2. receipt data version과 prefix data version 일치
3. 각 source object path, bytes와 SHA-256 형식
4. map 두 개를 먼저 검증하고 `normal` available 참조 집합 추출
5. 그 참조 집합에 필요한 pack만 선택하고 모든 다운로드의 bytes/SHA-256 일치
6. tar.zst member가 단일 `<source-sha256>.<ext>` 이름이고 실제 바이트 hash와 일치

sample은 각 locale map의 normal available source hash 오름차순 앞 N장을 선택한 뒤 두 locale 집합을 합친다. 동일 hash가 두 locale에서 선택되면 한 번만 처리하고 두 reference를 보존한다. 동일 입력과 N은 같은 선택 집합을 만든다. `max_images=0`은 map이 참조한 모든 고유 normal image가 누락 없이 선택돼야 한다.

## 4. 도구와 변환

workflow가 공식 Real-ESRGAN portable archive v0.2.5.0을 고정 URL에서 내려받고 다음 SHA-256을 검증한다.

- archive: `abc02804e17982a3be33675e4d471e91ea374e65b70167abc09e31acb412802d`
- executable: `07e49f7cbb4ede01ae4dd4c399d3a7e5846e3d2085c3128eff881e55cb7b1a0c`
- `realesrgan-x4plus.param`: `35330ececcea33b6c397a72548e788d5d53becee4734c50b7fada36e89f10a86`
- `realesrgan-x4plus.bin`: `713ee713b0353afaa27976f0563a64a5043bd70b9bd8936c2e26e25ebcdbcddf`

Python 의존성은 고정 버전 `boto3 1.43.65`, `Pillow 12.3.0`, `zstandard 0.25.0`을 runner temp에 설치한다. source member는 Pillow로 디코드해 source hash 이름의 RGBA PNG inference 입력으로 정규화한다. Real-ESRGAN은 GPU 0, 모델 `realesrgan-x4plus`, scale 4, PNG 출력을 사용한다.

후처리는 다음 순서다.

1. x4 RGB/RGBA 결과를 원본 `(width × 2, height × 2)`로 Lanczos 축소
2. 원본 RGBA의 alpha channel을 같은 크기로 Lanczos 확대
3. alpha를 x2 결과에 덮어쓰기
4. RGBA PNG로 저장
5. 디코드, mode, 정확한 x2 dimensions, bytes와 SHA-256 검증

알파가 없는 JPEG source는 불투명 alpha 255가 적용된다. x4 중간 산출물은 성공과 실패 모두 job cleanup 대상이다.

## 5. 파생 map, pack과 receipt

파생 map은 `maps/normal-realesrgan-x2.json.zst` 하나다. source hash 오름차순 asset record마다 locale owner, source pack/member/hash/bytes/dimensions와 output pack/member/hash/bytes/dimensions를 기록한다. output member는 `<output-sha256>.png`다.

locale별 output pack 이름은 다음과 같다.

- `packs/ko_KR-normal-realesrgan-x2.tar.zst`
- `packs/en_US-normal-realesrgan-x2.tar.zst`

tar member는 output hash 오름차순이고 mtime/uid/gid/user/group/mode를 고정한다. zstd는 level 3, single-thread와 frame checksum을 사용한다. 생성 직후 다시 해제해 member path, size와 hash를 전부 검증한다.

receipt schema version은 1이며 다음을 포함한다.

- source candidate prefix와 source receipt SHA-256
- data version, GitHub run ID/attempt, sample limit와 complete/partial mode
- source selected count와 locale별 count
- tool/model/Python dependency version과 hash
- transform scale/downsample/alpha policy
- output map/pack path, bytes, SHA-256, member count와 unpacked bytes
- derived candidate prefix

시간, 로컬 절대경로와 비밀값은 receipt에 넣지 않는다.

## 6. R2 발행과 실패 계약

출력 경로는 source와 분리한다.

```text
candidates/derived-images/realesrgan-x2/<data-version>/runs/<run-id>-<attempt>/
├─ packs/
├─ maps/normal-realesrgan-x2.json.zst
└─ receipt.json
```

pack과 map을 먼저 업로드하고 새 임시 경로로 모두 다시 내려받아 size/SHA-256과 archive member를 검증한다. 그 뒤 receipt를 마지막에 업로드하고 다시 내려받아 exact bytes를 확인한다. receipt가 없거나 receipt가 고정한 object가 불일치하면 완료 candidate가 아니다. 기존 source/derived candidate, production pointer와 다른 실행의 object는 덮어쓰거나 삭제하지 않는다.

## 7. 실행과 보안

- timeout 120분, concurrency 1, `cancel-in-progress: false`
- runner 도구·GPU hash/version 검증 실패 시 inference 전 종료
- R2 access key는 job 환경에만 주입하고 map, receipt, 로그에 기록하지 않음
- 구조화 로그는 stage, locale, count, elapsed, aggregate bytes와 안정된 오류 종류만 기록
- GitHub Artifact에는 receipt, map과 안전한 로그만 7일 보존하고 대형 pack은 중복 보존하지 않음
- 사용자가 등록한 모든 외부 contributor workflow 승인 정책과 read-only `GITHUB_TOKEN`을 전제로 함

## 8. 완료 조건

- fixture가 source receipt/path/hash/tar 변조, sample 선택, x2 dimensions/alpha, deterministic metadata와 receipt-last 계약을 검증한다.
- 20장 live smoke가 self-hosted RTX 4090에서 성공하고 R2 derived receipt를 마지막에 남긴다.
- 같은 workflow가 `max_images=0`으로 전체 normal image set을 처리할 수 있다.
- 원격 pack/map/receipt 재검증이 통과하고 source candidate와 public pointer는 변하지 않는다.
- `npm run check`, `/va HCL-016`, `npm run merge:check -- HCL-016`가 통과한다.
