#!/usr/bin/env bash
set -euo pipefail

python3 - <<'PY'
from html.parser import HTMLParser
from pathlib import Path

root = Path("docs")
page = root / "index.html"
if not page.is_file():
    raise SystemExit("docs/index.html is missing")

class Document(HTMLParser):
    def __init__(self):
        super().__init__()
        self.ids = set()
        self.local_links = []
        self.assets = []

    def handle_starttag(self, tag, attrs):
        values = dict(attrs)
        if values.get("id"):
            self.ids.add(values["id"])
        if tag == "a" and values.get("href", "").startswith("#"):
            self.local_links.append(values["href"][1:])
        if tag == "script" and values.get("src"):
            self.assets.append(values["src"])
        if tag == "link" and values.get("rel") == "stylesheet" and values.get("href"):
            self.assets.append(values["href"])

document = Document()
document.feed(page.read_text(encoding="utf-8"))
for target in document.local_links:
    if target not in document.ids:
        raise SystemExit(f"docs link target is missing: #{target}")
for asset in document.assets:
    if not (root / asset).is_file():
        raise SystemExit(f"docs asset is missing: {asset}")
for required in ("#main", "#workflow", "#capabilities", "#tool-api", "operation-grid"):
    if required.startswith("#") and required[1:] not in document.ids:
        raise SystemExit(f"required docs anchor is missing: {required}")
    if required == "operation-grid" and "operation-grid" not in page.read_text(encoding="utf-8"):
        raise SystemExit("operation index is missing")
print("MediaForge docs validation: PASS")
PY
