<script lang="ts">
  const version = "0.1.0";
</script>

<main>
  <header>
    <h1>Yonodex</h1>
    <p class="tagline">Decentralized Exchange</p>
  </header>

  <section class="status">
    <div class="card">
      <h2>Desktop Client</h2>
      <p>Version {version}</p>
      <p class="muted">Wallet and portfolio coming in 1.B</p>
    </div>
  </section>

  <footer>
    <p>AGPL-3.0 · Yonodex Team</p>
  </footer>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background: #0f0f12;
    color: #e8e8ec;
  }

  main {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 2rem;
    gap: 3rem;
  }

  header {
    text-align: center;
  }

   h1 {
    font-size: 3rem;
    margin: 0;
    line-height: 1.4;
    padding: 0.2em 0;
    background: linear-gradient(135deg, #a78bfa, #7c3aed);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .tagline {
    color: #8b8b94;
    margin-top: 0.5rem;
    font-size: 1.1rem;
  }

  .card {
    background: #1a1a1f;
    border: 1px solid #2a2a32;
    border-radius: 12px;
    padding: 2rem;
    max-width: 400px;
    text-align: center;
  }

  .card h2 {
    margin: 0 0 1rem 0;
    color: #e8e8ec;
  }

  .card p {
    margin: 0.5rem 0;
    color: #b8b8c0;
  }

  .muted {
    color: #6b6b74 !important;
    font-size: 0.9rem;
  }

  footer {
    color: #6b6b74;
    font-size: 0.85rem;
  }
</style>