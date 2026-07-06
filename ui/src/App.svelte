<script lang="ts">
  import WorkspaceShell from './lib/components/WorkspaceShell.svelte';
  import { bootAuth } from './lib/stores/auth.svelte.ts';
  import { onMount } from 'svelte';

  onMount(async () => {
      await bootAuth();
  });
</script>

<WorkspaceShell />
