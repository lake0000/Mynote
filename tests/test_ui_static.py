import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class UiStaticTests(unittest.TestCase):
    def test_page_contains_core_diary_controls(self):
        html = (ROOT / "static" / "index.html").read_text(encoding="utf-8")
        for marker in [
            'id="newNoteBtn"',
            'id="searchInput"',
            'id="noteList"',
            'id="titleInput"',
            'id="editor"',
            'contenteditable="true"',
            'id="backupBtn"',
            'id="restoreBtn"',
            'id="trashBtn"',
        ]:
            self.assertIn(marker, html)

    def test_toolbar_contains_required_editor_actions(self):
        html = (ROOT / "static" / "index.html").read_text(encoding="utf-8")
        for command in [
            'data-command="bold"',
            'data-command="italic"',
            'data-command="insertUnorderedList"',
            'data-value="blockquote"',
            'data-command="insertHorizontalRule"',
            'data-command="removeFormat"',
        ]:
            self.assertIn(command, html)

    def test_visual_design_tokens_are_present(self):
        css = (ROOT / "static" / "styles.css").read_text(encoding="utf-8")
        self.assertIn("backdrop-filter: blur", css)
        self.assertIn("border-radius", css)
        self.assertIn("--rose", css)
        self.assertIn("--mint", css)
        self.assertIn("rgba(", css)
        self.assertIn("@media (max-width: 820px)", css)

    def test_autosave_and_before_unload_are_implemented(self):
        js = (ROOT / "static" / "app.js").read_text(encoding="utf-8")
        self.assertIn("setTimeout(saveNow, 800)", js)
        self.assertIn("beforeunload", js)
        self.assertIn("sendBeacon", js)
        self.assertIn("quick-save", js)
        self.assertIn("window.__TAURI__", js)
        self.assertNotIn("await loadNotes(false)", js)

    def test_click_to_run_files_are_present(self):
        for filename in ["Mynote.exe", "package.json", "build_exe.ps1"]:
            self.assertTrue((ROOT / filename).is_file(), filename)

    def test_tauri_packaging_files_are_present(self):
        for filename in ["src-tauri/Cargo.toml", "src-tauri/tauri.conf.json", "src-tauri/src/main.rs"]:
            self.assertTrue((ROOT / filename).is_file(), filename)
        config = (ROOT / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8")
        self.assertIn('"withGlobalTauri": true', config)
        self.assertIn('"frontendDist": "../static"', config)
        rust = (ROOT / "src-tauri" / "src" / "main.rs").read_text(encoding="utf-8")
        self.assertIn("rusqlite", rust)
        self.assertIn("std::env::current_exe()", rust)


if __name__ == "__main__":
    unittest.main()
