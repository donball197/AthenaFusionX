<script lang="ts">
  import WorkspaceShell from './lib/components/WorkspaceShell.svelte';
  import { bootAuth, authState } from './lib/stores/auth.svelte.ts';
  import { onMount } from 'svelte';

  onMount(async () => {
      await bootAuth();
  });
</script>

{#if authState.isAuthenticated}
  <WorkspaceShell />
{:else}
  <div style="display:flex; justify-content:center; align-items:center; height:100vh; background:#000; color:#0f0; font-family:monospace; flex-direction:column; gap:20px;">
    <h1>NEXUS SOVEREIGN UPLINK</h1>
    <p>Awaiting master key authorization...</p>
    <button on:click={bootAuth} style="padding:10px 20px; background:#0f0; color:#000; border:none; cursor:pointer; font-family:monospace; font-weight:bold;">
      RE-AUTHENTICATE
    </button>
  </div>
{/if}
