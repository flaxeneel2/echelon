<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  // this is temporary, just to let me test backend while others work on the frontend
  window.core = {}
  window.core.invoke = invoke
  window.core.listen = listen

  let username = $state("");
  let password = $state("");
  let homeserver = $state("https://matrix.org");
  let error = $state("");
  let loading = $state(false);

  async function login() {
    error = "";
    loading = true;
    try {
      await invoke("login", { username, password, homeserver });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
</script>

<main class="container">
  <div class="login-card">
    <h1>echelon</h1>
    <p class="subtitle">Sign in to your Matrix account</p>

    <form onsubmit={(e) => { e.preventDefault(); login(); }}>
      <div class="field">
        <label for="username">Username</label>
        <input
          id="username"
          type="text"
          placeholder="@user:matrix.org"
          bind:value={username}
          disabled={loading}
        />
      </div>

      <div class="field">
        <label for="password">Password</label>
        <input
          id="password"
          type="password"
          placeholder="••••••••"
          bind:value={password}
          disabled={loading}
        />
      </div>

      <div class="field">
        <label for="homeserver">Homeserver</label>
        <input
          id="homeserver"
          type="text"
          placeholder="https://matrix.org"
          bind:value={homeserver}
          disabled={loading}
        />
      </div>

      {#if error}
        <p class="error">{error}</p>
      {/if}

      <button type="submit" disabled={loading}>
        {loading ? "Signing in..." : "Sign in"}
      </button>
    </form>
  </div>
</main>

<style>
  :root {
    font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
    font-size: 16px;
    line-height: 24px;
    font-weight: 400;
    color: #f0f0f0;
    background-color: #0a0a0a;
    font-synthesis: none;
    text-rendering: optimizeLegibility;
    -webkit-font-smoothing: antialiased;
  }

  .container {
    margin: 0;
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    background-color: #0a0a0a;
  }

  .login-card {
    background-color: #111111;
    border: 1px solid #1e3a5f;
    border-radius: 12px;
    padding: 2.5rem;
    width: 100%;
    max-width: 400px;
    box-shadow: 0 0 40px rgba(30, 100, 220, 0.08);
  }

  h1 {
    margin: 0 0 0.25rem 0;
    font-size: 2rem;
    font-weight: 700;
    color: #4a90e2;
    text-align: center;
    letter-spacing: 0.05em;
  }

  .subtitle {
    text-align: center;
    color: #666;
    font-size: 0.9rem;
    margin: 0 0 2rem 0;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  label {
    font-size: 0.85rem;
    color: #999;
    font-weight: 500;
  }

  input {
    background-color: #1a1a1a;
    border: 1px solid #2a2a2a;
    border-radius: 8px;
    padding: 0.65rem 0.9rem;
    font-size: 0.95rem;
    color: #f0f0f0;
    transition: border-color 0.2s;
    outline: none;
    width: 100%;
    box-sizing: border-box;
    font-family: inherit;
  }

  input:focus {
    border-color: #2563eb;
  }

  input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  input::placeholder {
    color: #444;
  }

  button {
    margin-top: 0.5rem;
    background-color: #2563eb;
    color: #fff;
    border: none;
    border-radius: 8px;
    padding: 0.75rem;
    font-size: 1rem;
    font-weight: 600;
    font-family: inherit;
    cursor: pointer;
    transition: background-color 0.2s, opacity 0.2s;
  }

  button:hover:not(:disabled) {
    background-color: #1d4ed8;
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .error {
    color: #f87171;
    font-size: 0.85rem;
    text-align: center;
    margin: 0;
    padding: 0.5rem;
    background-color: rgba(248, 113, 113, 0.08);
    border-radius: 6px;
    border: 1px solid rgba(248, 113, 113, 0.2);
  }
</style>