<script lang="ts">
  type Variant = "primary" | "ghost" | "danger" | "outline";
  type Size = "sm" | "md";

  type Props = {
    variant?: Variant;
    size?: Size;
    disabled?: boolean;
    type?: "button" | "submit" | "reset";
    onclick?: (e: MouseEvent) => void;
    children: import("svelte").Snippet;
    [key: string]: unknown;
  };

  let {
    variant = "outline",
    size = "md",
    disabled = false,
    type = "button",
    onclick,
    children,
    ...rest
  }: Props = $props();
</script>

<button
  {type}
  {disabled}
  class="btn btn--{variant} btn--{size}"
  onclick={onclick}
  {...rest}
>
  {@render children()}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 500;
    line-height: 1;
    border: 1px solid transparent;
    transition: background 0.1s, border-color 0.1s, opacity 0.1s;
    white-space: nowrap;
    cursor: pointer;
  }

  .btn--md { padding: 6px 12px; height: 30px; }
  .btn--sm { padding: 4px 9px; height: 26px; font-size: 12px; }

  /* Primary */
  .btn--primary {
    background: var(--accent);
    color: #fff;
    border-color: var(--accent);

    &:hover:not(:disabled) {
      background: var(--accent-light);
      border-color: var(--accent-light);
    }

    &:active:not(:disabled) {
      filter: brightness(0.9);
    }
  }

  /* Outline */
  .btn--outline {
    background: var(--surface);
    color: var(--text);
    border-color: var(--line-strong);

    &:hover:not(:disabled) {
      background: var(--surface-hover);
      border-color: var(--line-strong);
    }

    &:active:not(:disabled) {
      background: var(--surface-active);
    }
  }

  /* Ghost */
  .btn--ghost {
    background: transparent;
    color: var(--text-muted);
    border-color: transparent;

    &:hover:not(:disabled) {
      background: var(--surface-hover);
      color: var(--text);
    }
  }

  /* Danger */
  .btn--danger {
    background: var(--danger-soft);
    color: var(--danger);
    border-color: var(--danger-border);

    &:hover:not(:disabled) {
      background: var(--danger);
      color: #fff;
    }
  }
</style>
