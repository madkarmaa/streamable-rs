#!/usr/bin/env python3

import os
import shutil
import tempfile
from pathlib import Path
from urllib.request import Request, urlopen
from zipfile import ZipFile

URL = "https://file.garden/ZzSF-2oH_UI6UVla/streamable.com.zip"
DEST = Path("dump").resolve()

DEST.mkdir(parents=True, exist_ok=True)

headers = {
    "User-Agent": "Mozilla/5.0",
    "Accept": "*/*",
}

def long_path(path: Path) -> str:
    resolved = str(path.resolve())

    if os.name == "nt":
        if resolved.startswith("\\\\"):
            return "\\\\?\\UNC\\" + resolved[2:]
        return "\\\\?\\" + resolved

    return resolved


with tempfile.TemporaryDirectory() as tmp:
    zip_path = Path(tmp) / "streamable.com.zip"

    print(f"Downloading {URL}...")

    request = Request(URL, headers=headers)

    with urlopen(request) as response, open(zip_path, "wb") as f:
        shutil.copyfileobj(response, f)

    print(f"Extracting to {DEST}...")

    with ZipFile(zip_path) as archive:
        for member in archive.infolist():
            target = DEST / member.filename

            try:
                target.resolve().relative_to(DEST)
            except ValueError:
                raise RuntimeError(f"Unsafe path in ZIP: {member.filename}")

            target_str = long_path(target)

            if member.is_dir():
                os.makedirs(target_str, exist_ok=True)
                continue

            parent_str = long_path(target.parent)
            os.makedirs(parent_str, exist_ok=True)

            with archive.open(member) as src, open(target_str, "wb") as dst:
                shutil.copyfileobj(src, dst)

print("Done.")