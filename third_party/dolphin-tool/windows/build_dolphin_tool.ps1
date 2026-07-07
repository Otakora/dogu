<#
.SYNOPSIS
  Builds DolphinTool.exe from the official Dolphin source and drops it at
  third_party/dolphin-tool/windows/DolphinTool.exe.

.DESCRIPTION
  DolphinTool is GPL-2.0+ (part of the Dolphin emulator). Building and
  redistributing it is allowed as long as the corresponding source is offered —
  see third_party/THIRD-PARTY-LICENSES.md. This mirrors the chdman approach.

  Requirements (install once):
    - Git
    - CMake 3.22+
    - Visual Studio 2022 with the "Desktop development with C++" workload
      (MSVC v143 toolset + Windows 10/11 SDK)

  Usage (from a normal PowerShell prompt):
    pwsh -File build_dolphin_tool.ps1
    # or: powershell -ExecutionPolicy Bypass -File build_dolphin_tool.ps1

  Output:
    third_party/dolphin-tool/windows/DolphinTool.exe
#>

$ErrorActionPreference = "Stop"

# Pinned Dolphin release tag. Update to match the version you want to ship, then
# also update third_party/dolphin-tool/metadata.json.
$DolphinTag = if ($env:DOLPHIN_TAG) { $env:DOLPHIN_TAG } else { "2606" }

$ScriptDir  = Split-Path -Parent $MyInvocation.MyCommand.Definition
$WorkDir    = Join-Path (Split-Path -Parent $ScriptDir) ".build-windows"
$DolphinDir = Join-Path $WorkDir "dolphin"
$BuildDir   = Join-Path $DolphinDir "build"
$OutputPath = Join-Path $ScriptDir "DolphinTool.exe"

Write-Host "=== Dogu / DolphinTool build (Windows) ==="
Write-Host "Dolphin tag : $DolphinTag"
Write-Host "Output      : $OutputPath"
Write-Host ""

New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null

# ── 1. Clone Dolphin (recursive submodules for bundled Externals) ────────────
if (-not (Test-Path (Join-Path $DolphinDir ".git"))) {
  Write-Host "--- Cloning Dolphin ($DolphinTag)..."
  git clone --depth 1 --recurse-submodules --shallow-submodules `
    -b $DolphinTag https://github.com/dolphin-emu/dolphin.git $DolphinDir
  if ($LASTEXITCODE -ne 0) { throw "git clone failed" }
} else {
  Write-Host "--- Reusing existing clone at $DolphinDir"
  git -C $DolphinDir fetch --depth 1 origin $DolphinTag
  git -C $DolphinDir checkout $DolphinTag
  git -C $DolphinDir submodule update --init --recursive --depth 1
}

# ── 2. Configure (Visual Studio generator, no Qt/GUI) ────────────────────────
Write-Host "--- Configuring (CMake, Visual Studio 2022, no Qt)..."
cmake -S $DolphinDir -B $BuildDir -G "Visual Studio 17 2022" -A x64 `
  -DENABLE_QT=OFF `
  -DENABLE_TESTS=OFF `
  -DENABLE_ANALYTICS=OFF `
  -DENABLE_AUTOUPDATE=OFF `
  -DUSE_DISCORD_PRESENCE=OFF `
  -DENABLE_CLI_TOOL=ON
if ($LASTEXITCODE -ne 0) {
  throw "CMake configure failed. If Windows CMake support is problematic, open Source/dolphin-emu.sln in Visual Studio and build the 'DolphinTool' project (Release x64) instead."
}

# ── 3. Build only the dolphin-tool target ────────────────────────────────────
Write-Host "--- Building dolphin-tool..."
cmake --build $BuildDir --config Release --target dolphin-tool
if ($LASTEXITCODE -ne 0) { throw "Build failed" }

# ── 4. Locate and copy the binary ────────────────────────────────────────────
$built = Get-ChildItem -Path $BuildDir -Recurse -Filter "DolphinTool.exe" -ErrorAction SilentlyContinue |
  Select-Object -First 1
if (-not $built) {
  throw "DolphinTool.exe not found under $BuildDir"
}
Copy-Item -Force $built.FullName $OutputPath

Write-Host ""
Write-Host "=== Build finished ==="
Write-Host "Binary: $OutputPath"
Write-Host "Verify:  & '$OutputPath' convert --help"
Write-Host "Then commit it:"
Write-Host "  git add third_party/dolphin-tool/windows/DolphinTool.exe"
Write-Host "  git commit -m 'chore: bundle DolphinTool Windows binary'"
