svelte
<script>
  import { onMount } from 'svelte';

  let message = 'Loading...';

  onMount(async () => {
    try {
      const response = await fetch('/api/hello');
      const data = await response.json();
      message = data.message;
    } catch (error) {
      message = 'Error fetching data: ' + error;
    }
  });
</script>

<h1>{message}</h1>