<script>
  import { onMount } from 'svelte';

  let currentTime = '';

  onMount(() => {
    const intervalId = setInterval(() => {
      const now = new Date();
      currentTime = now.toLocaleTimeString();
    }, 1000);

    return () => clearInterval(intervalId);
  });
</script>

<div class="terminal-app">
  <p style="color: #00FF00; font-family: monospace; font-size: 24px;">{currentTime}</p>
</div>

<style>
  .terminal-app {
    background-color: #000;
    padding: 20px;
    border-radius: 5px;
    width: fit-content;
    margin: 0 auto;
  }
</style>