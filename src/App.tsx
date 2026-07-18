import { invoke } from "@tauri-apps/api/core";
import { createSignal, For, onMount } from "solid-js";

import { APP_NAME, SETUP_PARTS } from "./app-meta";

type BackendState = "확인 중" | "연결됨" | "브라우저 미리보기";

export default function App() {
  const [backendState, setBackendState] = createSignal<BackendState>("확인 중");

  onMount(() => {
    void invoke<string>("health_check")
      .then(() => setBackendState("연결됨"))
      .catch(() => setBackendState("브라우저 미리보기"));
  });

  return (
    <main class="shell">
      <section class="hero">
        <p class="eyebrow">PERSONAL DECK WORKBENCH</p>
        <h1>{APP_NAME}</h1>
        <p class="lead">
          하스스톤 카드 수집, 고급 조건 검색, 덱 구성과 덱 코드 내보내기를 위한
          로컬 데스크톱 도구입니다.
        </p>
        <div class="status-row">
          <span class="status-dot" aria-hidden="true" />
          <strong>개발 환경 준비됨</strong>
          <span>Rust 백엔드: {backendState()}</span>
        </div>
      </section>

      <section class="setup-grid" aria-label="프로젝트 구성">
        <For each={SETUP_PARTS}>
          {(part, index) => (
            <article>
              <span>0{index() + 1}</span>
              <h2>{part}</h2>
            </article>
          )}
        </For>
      </section>

      <section class="next-step">
        <p>다음 구현 지점</p>
        <h2>공식 카드 수집기를 Rust 서비스로 옮기고 로컬 저장소를 연결합니다.</h2>
      </section>
    </main>
  );
}
