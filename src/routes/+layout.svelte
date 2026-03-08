<script lang="ts">
  import { initTauriCore } from "$lib/utils";
  import { onMount } from "svelte";

  onMount(() => {
    initTauriCore();
  });

  const { children } = $props();
</script>

{@render children()}
