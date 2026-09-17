<script lang="ts">
  import { onMount } from "svelte";
  import {
    dbIsInitialized,
    unlockDb,
    savePasswordHint,
    loadPasswordHint,
    resetLocalData,
  } from "../db";

  let { onunlock }: { onunlock?: () => void } = $props();

  let initialized = $state<boolean | null>(null);
  let password = $state("");
  let confirm = $state("");
  let hint = $state("");
  let existingHint = $state<string | null>(null);
  let failedAttempts = $state(0);
  let submitting = $state(false);
  let error = $state<string | null>(null);
  let showResetConfirm = $state(false);

  const HINT_REVEAL_THRESHOLD = 3;

  onMount(async () => {
    try {
      initialized = await dbIsInitialized();
      if (initialized) {
        existingHint = await loadPasswordHint();
      }
    } catch {
      initialized = true;
    }
  });

  async function handleSubmit(e: Event) {
    e.preventDefault();
    if (submitting) return;

    if (!initialized) {
      if (password.length < 8) {
        error = "Password must be at least 8 characters.";
        return;
      }
      if (password !== confirm) {
        error = "Passwords do not match.";
        return;
      }
    } else {
      if (!password) return;
    }

    submitting = true;
    error = null;
    try {
      await unlockDb(password);
      if (!initialized && hint.trim()) {
        try {
          await savePasswordHint(hint.trim());
        } catch {
          // Non-fatal
        }
      }
      password = "";
      confirm = "";
      hint = "";
      failedAttempts = 0;
      onunlock?.();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.toLowerCase().includes("wrong password")) {
        failedAttempts += 1;
        error = "Wrong password. Try again.";
      } else {
        error = msg;
      }
    } finally {
      submitting = false;
    }
  }

  async function handleReset() {
    try {
      await resetLocalData();
      initialized = false;
      existingHint = null;
      password = "";
      confirm = "";
      hint = "";
      failedAttempts = 0;
      error = null;
      showResetConfirm = false;
    } catch (e) {
      error = e instanceof Error ? e.message : "Failed to reset local data";
    }
  }
</script>

<div class="unlock">
  {#if initialized === null}
    <p class="hint">Loading...</p>
  {:else if !initialized}
    <h2>Create Master Password</h2>
    <p class="hint">
      This password encrypts all local Yonodex data.
      <br />
      <span class="small warning-text">
        ⚠ There is no recovery. If you forget it, your data is lost.
      </span>
    </p>

    <form onsubmit={handleSubmit}>
      <input
        type="password"
        bind:value={password}
        placeholder="Choose a strong password (min 8 chars)"
        autocomplete="new-password"
        disabled={submitting}
      />
      <input
        type="password"
        bind:value={confirm}
        placeholder="Confirm password"
        autocomplete="new-password"
        disabled={submitting}
      />
      <input
        type="text"
        bind:value={hint}
        placeholder="Hint (optional, stored in plaintext)"
        autocomplete="off"
        disabled={submitting}
        maxlength="80"
      />
      <button type="submit" disabled={submitting || !password || !confirm}>
        {submitting ? "Creating..." : "Create & Unlock"}
      </button>
    </form>
  {:else}
    <h2>Unlock Yonodex</h2>
    <p class="hint">Enter your master password to decrypt local data.</p>

    {#if existingHint && failedAttempts >= HINT_REVEAL_THRESHOLD}
      <p class="hint-revealed">Hint: {existingHint}</p>
    {/if}

    <form onsubmit={handleSubmit}>
      <input
        type="password"
        bind:value={password}
        placeholder="Master password"
        autocomplete="current-password"
        disabled={submitting}
      />
      <button type="submit" disabled={submitting || !password}>
        {submitting ? "Unlocking..." : "Unlock"}
      </button>
    </form>

    {#if failedAttempts > 0 && failedAttempts < HINT_REVEAL_THRESHOLD}
      <p class="hint-small">
        Attempts remaining before hint shows: {HINT_REVEAL_THRESHOLD - failedAttempts}
      </p>
    {/if}

    <div class="reset-section">
      {#if showResetConfirm}
        <p class="hint-small warning-text">
          This will delete the local database, salt, and hint permanently.
        </p>
        <div class="reset-actions">
          <button class="reset-confirm" onclick={handleReset}>Yes, reset</button>
          <button class="reset-cancel" onclick={() => (showResetConfirm = false)}>
            Cancel
          </button>
        </div>
      {:else}
        <button class="reset-link" onclick={() => (showResetConfirm = true)}>
          Forgot password? Reset local data
        </button>
      {/if}
    </div>
  {/if}

  {#if error}
    <p class="error">{error}</p>
  {/if}
</div>

<style>
  .unlock {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    min-width: 340px;
    padding: 1rem;
  }

  h2 {
    margin: 0;
    color: #e8e8ec;
    font-size: 1.2rem;
  }

  .hint {
    color: #8b8b94;
    font-size: 0.85rem;
    text-align: center;
    line-height: 1.5;
    margin: 0;
  }

  .hint-small {
    color: #6b6b74;
    font-size: 0.75rem;
    text-align: center;
    margin: 0;
  }

  .hint-revealed {
    color: #a78bfa;
    font-size: 0.85rem;
    text-align: center;
    margin: 0;
    padding: 0.5rem 0.75rem;
    background: rgba(167, 139, 250, 0.08);
    border: 1px solid rgba(167, 139, 250, 0.2);
    border-radius: 6px;
  }

  .hint .small {
    font-size: 0.75rem;
    color: #6b6b74;
  }

  .warning-text {
    color: #f59e0b !important;
    display: block;
    margin-top: 0.5rem;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    width: 100%;
  }

  input {
    background: #131318;
    border: 1px solid #2a2a32;
    border-radius: 8px;
    padding: 0.75rem 1rem;
    font-size: 0.95rem;
    color: #e8e8ec;
    font-family: inherit;
    outline: none;
  }

  input:focus {
    border-color: #a78bfa;
  }

  input:disabled {
    opacity: 0.5;
  }

  form button {
    background: linear-gradient(135deg, #a78bfa, #7c3aed);
    color: white;
    border: none;
    border-radius: 8px;
    padding: 0.75rem 1.5rem;
    font-size: 0.95rem;
    font-weight: 600;
    cursor: pointer;
    transition: opacity 0.2s;
  }

  form button:hover:not(:disabled) {
    opacity: 0.9;
  }

  form button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .reset-section {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.5rem;
    width: 100%;
  }

  .reset-link {
    background: transparent;
    color: #6b6b74;
    border: none;
    font-size: 0.75rem;
    cursor: pointer;
    text-decoration: underline;
    padding: 0;
  }

  .reset-link:hover {
    color: #b8b8c0;
  }

  .reset-actions {
    display: flex;
    gap: 0.5rem;
    width: 100%;
  }

  .reset-confirm {
    flex: 1;
    background: #7f1d1d;
    color: #fecaca;
    border: none;
    border-radius: 6px;
    padding: 0.5rem;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }

  .reset-confirm:hover {
    background: #991b1b;
  }

  .reset-cancel {
    flex: 1;
    background: transparent;
    color: #b8b8c0;
    border: 1px solid #4a4a55;
    border-radius: 6px;
    padding: 0.5rem;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .error {
    color: #f87171;
    font-size: 0.85rem;
    text-align: center;
    margin: 0;
  }
</style>