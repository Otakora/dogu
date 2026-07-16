import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { spawn, spawnSync } from "node:child_process";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const secretDir = resolve(root, ".dev-secrets");
const privateKeyPath = resolve(secretDir, "dogu-updater.key");
const publicKeyPath = resolve(root, ".dev-secrets", "dogu-updater.key.pub");
const tauriCli = resolve(root, "node_modules", "@tauri-apps", "cli", "tauri.js");

function ensureDevelopmentPublicKey() {
  const envPublicKey = process.env.DOGU_UPDATER_PUBLIC_KEY?.trim();
  if (envPublicKey) return envPublicKey;

  if (existsSync(publicKeyPath)) {
    return readFileSync(publicKeyPath, "utf8").trim();
  }

  if (existsSync(privateKeyPath)) {
    console.error("Dogu updater public key is missing, but the private development key exists.");
    console.error("Run `npm run updater:generate-dev-key -- --force` to rotate the local development keypair.");
    process.exit(1);
  }

  mkdirSync(secretDir, { recursive: true });
  console.log("Generating local development updater keypair in .dev-secrets...");
  const result = spawnSync(process.execPath, [tauriCli, "signer", "generate", "--ci", "--write-keys", privateKeyPath], {
    cwd: root,
    stdio: "inherit",
  });
  if (result.status !== 0 || !existsSync(publicKeyPath)) {
    console.error("Could not generate Dogu development updater keypair.");
    process.exit(result.status ?? 1);
  }
  return readFileSync(publicKeyPath, "utf8").trim();
}

const publicKey = ensureDevelopmentPublicKey();

if (!publicKey) {
  console.error("Dogu updater public key is missing for development.");
  console.error("Run `npm run updater:generate-dev-key` or set DOGU_UPDATER_PUBLIC_KEY.");
  process.exit(1);
}

const updaterConfig = JSON.stringify({
  plugins: {
    updater: {
      pubkey: publicKey,
      windows: {
        installMode: "passive",
      },
    },
  },
});

const child = spawn(process.execPath, [tauriCli, "dev", "--config", updaterConfig, ...process.argv.slice(2)], {
  cwd: root,
  env: {
    ...process.env,
    DOGU_UPDATER_PUBLIC_KEY: publicKey,
  },
  stdio: "inherit",
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 0);
});
