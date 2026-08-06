# HCL-014 R2 Raw 후보 업로드 계약

## 목표

수동 GitHub Actions 실행이 HCL-006 production CLI를 그대로 사용해 두 locale 패키지를 완성·검증한 뒤, Cloudflare R2에는 Raw zstd 두 객체만 고유 candidate 경로로 보존한다.

## 발행 경계

- 실행 trigger는 `workflow_dispatch` 하나다.
- 입력은 HCL-006 형식의 `data_version` 하나다.
- GitHub Actions artifact에는 manifest를 포함한 전체 검증 패키지를 7일 보존한다.
- R2에는 `raw/ko_KR.json.zst`, `raw/en_US.json.zst`만 업로드한다.
- normalized SQLite, manifest, 서명, 이미지, diff와 제어 pointer는 R2에 올리지 않는다.
- `stable.json`, `versions.json`, `current` 계열 객체를 만들거나 변경하지 않는다.

## R2 key

```text
candidates/raw/<data-version>/runs/<github-run-id>-<run-attempt>/raw/ko_KR.json.zst
candidates/raw/<data-version>/runs/<github-run-id>-<run-attempt>/raw/en_US.json.zst
```

GitHub run ID와 attempt를 포함하므로 정상 workflow 실행은 기존 candidate를 덮어쓰지 않는다. 운영 release 경로와 pointer-last 승격은 HCL-007 소비 검증 뒤 별도 CD 작업이 담당한다.

## 검증

업로드 전 package manifest의 locale, 상대 경로, byte size와 SHA-256을 실제 Raw 파일과 비교한다. 업로드 후 같은 S3 endpoint에서 두 객체를 다시 내려받아 동일 byte size와 SHA-256을 검사한다. 둘 중 하나라도 실패하면 workflow 전체를 실패시키며 운영 pointer는 없으므로 앱 노출 상태는 바뀌지 않는다.

## 자격증명

GitHub Actions Secrets:

- `BLIZZARD_CLIENT_ID`
- `BLIZZARD_CLIENT_SECRET`
- `R2_ACCESS_KEY_ID`
- `R2_SECRET_ACCESS_KEY`

GitHub Actions Variables:

- `R2_ACCOUNT_ID`
- `R2_BUCKET`

workflow permission은 `contents: read`만 사용한다. 자격증명은 build와 R2 단계의 process environment에만 주입하고 산출물, receipt와 로그에는 기록하지 않는다.
