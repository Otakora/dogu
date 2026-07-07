# Third-party tools bundled with Dogu

Dogu is free, open-source software (not sold). It invokes several external tools
as **separate processes** (mere aggregation), and bundles some of their binaries
for convenience. This document lists each bundled tool, its license, and — for
GPL-licensed tools — where to obtain the exact **corresponding source code**, as
required by the GNU General Public License.

Each tool's own license text is kept next to its binaries (e.g.
`chdman/windows/COPYING`, `7zip/windows/License.txt`). The full, canonical GPL-2.0
text is the standard one published by the FSF at
<https://www.gnu.org/licenses/old-licenses/gpl-2.0.txt> (also reproduced in the
upstream MAME source under `docs/legal/`).

| Tool | Version | License | Bundled here | Source |
|------|---------|---------|--------------|--------|
| chdman (MAME) | 0.288 | GPL-2.0 | yes (Win + Linux) | see offer below |
| 7-Zip | 26.01 | LGPL-2.1 + unRAR restriction | yes | https://www.7-zip.org/download.html |
| DolphinTool | 2606 | GPL-2.0-or-later | yes, built from source (Win + Linux) | see offer below |
| nod (RVZ engine) | 2.0.0-alpha.9 | MIT OR Apache-2.0 | compiled into Dogu | https://github.com/encounter/nod |

## Why some binaries live in this repo and others don't

- **chdman**, **7-Zip** and **DolphinTool** are kept here. None has a stable,
  standalone, official binary download suitable for redistribution: chdman ships
  only inside the ~250 MB MAME installer (and must be compiled on Linux),
  DolphinTool ships only inside full Dolphin releases with per-build versioned
  URLs, and 7-Zip is also the tool Dogu uses to extract archives. The GPL tools
  (chdman, DolphinTool) are **built from source** via the scripts next to them
  and committed; the corresponding source is offered below.

## Written offer for source (GPL-2.0 §3)

For every GPL-licensed binary distributed in this repository (**chdman** and
**DolphinTool**), the complete corresponding source code is available:

- **chdman / MAME 0.288** — official source release:
  https://github.com/mamedev/mame/releases/download/mame0288/mame0288s.exe
  (self-extracting archive) and the tagged tree
  https://github.com/mamedev/mame/tree/mame0288
- **DolphinTool / Dolphin 2606** — tagged source tree:
  https://github.com/dolphin-emu/dolphin/tree/2606 (build with the scripts in
  `dolphin-tool/`; the exact target is `dolphin-tool`).

In addition, for a period of **three (3) years** from the date you received this
copy, the Dogu maintainers will provide, on request, the complete corresponding
source for any GPL binary distributed here, on a physical medium or download, for
no more than the cost of distribution. Open an issue at the Dogu repository to
request it.

## unRAR note (7-Zip)

7-Zip includes RAR decompression under the unRAR license, which forbids using
that code to create a RAR-compatible archiver. Dogu only **extracts** RAR and
never creates RAR with this code, so it complies. See `7zip/windows/License.txt`.
