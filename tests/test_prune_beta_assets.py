import unittest

from tools.prune_beta_assets import obsolete_assets


class PruneBetaAssetsTests(unittest.TestCase):
    def setUp(self):
        self.version = "0.2.6-beta.1"
        v = self.version
        self.current = [
            f"dogu-windows-x64-{v}-setup.exe",
            f"dogu-windows-x64-{v}-setup.exe.sig",
            f"dogu-linux-x86_64-{v}.AppImage",
            f"dogu-linux-x86_64-{v}.AppImage.sig",
            f"dogu-linux-x86_64-{v}.deb",
            f"dogu-linux-x86_64-{v}.flatpak",
            "latest-beta.json",
            "SHA256SUMS.txt",
        ]

    def test_only_previous_versioned_beta_artifacts_are_selected(self):
        old = ["dogu-windows-x64-0.2.5-beta.5-setup.exe",
               "dogu-linux-x86_64-0.2.5-beta.5.AppImage.sig"]
        untouched = ["notes.txt", "dogu-windows-x64-0.2.5-setup.exe"]
        self.assertEqual(obsolete_assets(self.current + old + untouched, self.version), old)

    def test_missing_current_asset_blocks_cleanup(self):
        with self.assertRaises(ValueError):
            obsolete_assets(self.current[:-1], self.version)


if __name__ == "__main__":
    unittest.main()
