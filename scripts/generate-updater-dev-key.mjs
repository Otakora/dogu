import { existsSync, mkdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const secretDir = resolve(root, ".dev-secrets");
const privateKeyPath = resolve(secretDir, "dogu-updater.key");
const publicKeyPath = `${privateKeyPath}.pub`;
const tauriCli = resolve(root, "node_modules", "@tauri-apps", "cli", "tauri.js");
const force = process.argv.includes("--force");

mkdirSync(secretDir, { recursive: true });

if (!force && existsSync(privateKeyPath) && existsSync(publicKeyPath)) {
  console.log(`Development updater key already exists: ${privateKeyPath}`);
  console.log("Use `npm run updater:generate-dev-key -- --force` to replace it.");
  process.exit(0);
}

const result = spawnSync(
  process.execPath,
  [tauriCli, "signer", "generate", "--ci", "--write-keys", privateKeyPath, ...(force ? ["--force"] : [])],
  { cwd: root, stdio: "inherit" },
);

process.exit(result.status ?? 1);
