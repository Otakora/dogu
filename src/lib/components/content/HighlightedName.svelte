<script lang="ts">
  let { text, query }: { text: string; query: string } = $props();
  const parts = $derived.by(() => {
    if (!query) return [{ text, match: false }];
    const lower = text.toLowerCase(), needle = query.toLowerCase();
    const parts: Array<{ text: string; match: boolean }> = [];
    let offset = 0, index = lower.indexOf(needle);
    while (index >= 0) {
      if (index > offset) parts.push({ text: text.slice(offset, index), match: false });
      parts.push({ text: text.slice(index, index + needle.length), match: true });
      offset = index + needle.length;
      index = lower.indexOf(needle, offset);
    }
    if (offset < text.length) parts.push({ text: text.slice(offset), match: false });
    return parts;
  });
</script>
{#each parts as part}{#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}{/each}
<style>mark { background: #f5c85b; color: #241c0d; border-radius: 2px; padding: 0 1px; }</style>
