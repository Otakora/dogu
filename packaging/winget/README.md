# Winget packaging

Dogu uses `Otakora.Dogu` as its Windows Package Manager identifier.

The release workflow generates a singleton manifest for stable releases:

```text
Otakora.Dogu.yaml
```

That generated manifest points to the stable NSIS installer published on GitHub
Releases and includes the installer SHA256 required by winget.

## Release flow

1. Publish a stable Dogu release through `.github/workflows/build-artifacts.yml`.
2. Download the `Otakora.Dogu.yaml` artifact from the workflow or release assets.
3. Validate it locally:

```powershell
winget validate .\Otakora.Dogu.yaml
```

4. Test it in Windows Sandbox when practical.
5. Submit it to `microsoft/winget-pkgs`.

Beta releases are not submitted to winget. Users who choose Dogu's beta channel
receive beta updates through Dogu's own updater.

## Notes

- The installer type is `nullsoft` because Dogu's Windows installer is NSIS.
- Winget is a distribution/update channel for stable Windows installs, not a
  replacement for Dogu's in-app updater.
- SmartScreen reputation may still take time to improve. Winget helps with a
  familiar install path, public metadata and hash validation, but it is not a
  code-signing certificate.
