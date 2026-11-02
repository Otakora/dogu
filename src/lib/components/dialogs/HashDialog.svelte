<script lang="ts">
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import HashPanel, { type HashResult } from "./HashPanel.svelte";
  import { t } from "../../i18n/index.js";
  type Props = { paths: string[]; algorithms: string[]; autoStart: boolean; results?: HashResult[]; onclose: () => void };
  let { paths, algorithms, autoStart, results = [], onclose }: Props = $props();
</script>
<Modal title={t("hash.title")} width="740px" {onclose}>
  {#snippet children()}<HashPanel {paths} initialAlgorithms={algorithms} {autoStart} initialResults={results} />{/snippet}
  {#snippet footer()}<Button variant="ghost" onclick={onclose}>{t("common.close")}</Button>{/snippet}
</Modal>
