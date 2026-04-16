/**
 * Copyright 2026 Donald Ball
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


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