<script lang="ts">
  import "$lib/styles/login.css";

  // Svelte 5 States
  let username = $state("");
  let password = $state("");
  let homeserver = $state("matrix.org");
  let error = $state("");
  let loading = $state(false);
  let showPassword = $state(false);

  const oauthProviders = ["google", "github", "apple", "facebook", "gitlab"];

  async function login() {
    error = "";
    loading = true;
    try {
      const fullHomeserver = "https://"+ homeserver
      await window.core.invoke("login", { username, password, homeserver: fullHomeserver });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function OAuth_action(type: 'login' | 'register') {
    error = "";
    loading = true;
    try {
      await window.core.invoke(`oauth_${type}`, { homeserver });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
</script>

<main class="login-container">
  <section class="form-wrapper">
    <h1 class="title">echelon</h1>

    <form class="input-group" onsubmit={(e) => { e.preventDefault(); login(); }}>
      <input
              type="text"
              placeholder="username"
              bind:value={username}
              disabled={loading}
              class="input"
      />

      <div class="password-wrapper">
        <input
                type={showPassword ? "text" : "password"}
                placeholder="password"
                bind:value={password}
                disabled={loading}
                class="input"
        />
        <button type="button" class="password-toggle" onclick={() => showPassword = !showPassword} tabindex="-1">
          <img src={showPassword ? "/hide.svg" : "/see.svg"} class="icon-svg" alt="toggle password" />
        </button>
      </div>

      <div class="homeserver-wrapper">
        <input
                type="text"
                placeholder="homeserver"
                bind:value={homeserver}
                disabled={loading}
                class="input"
        />
        <div class="info-icon-wrapper">
          <img src="/info.svg" class="icon-svg" alt="info">
        </div>
      </div>

      {#if error}
        <p class="error-msg">{error}</p>
      {/if}

      <a href="/forgot" class="forgot-link">forgot your password?</a>

      <div class="button-stack">
        <button type="submit" class="button" disabled={loading}>
          {loading ? "..." : "login"}
        </button>

        <button type="button" class="button" onclick={() => OAuth_action('login')} disabled={loading}>
          oauth_login
        </button>
      </div>
    </form>

    <div class="oauth-icons">
      {#each oauthProviders as provider}
        <button class="oauth-icon" onclick={() => OAuth_action('login')} title={provider}>
          <img src="/{provider}.svg" alt={provider} class="icon-svg" />
        </button>
      {/each}
    </div>

    <button class="register-button" onclick={() => OAuth_action('register')} disabled={loading}>
      register
    </button>

    <a href="/chat" class="dev-chat-link">→ open chat (dev)</a>
  </section>
</main>