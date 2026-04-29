import json
import os
import pathlib
import urllib.request

URL = "https://models.dev/api.json"
USER_AGENT = "refact-lsp models.dev snapshot updater"


def main() -> None:
    root = pathlib.Path(__file__).resolve().parents[1]
    snapshot_path = root / "src" / "caps" / "models_dev_snapshot.json"
    request = urllib.request.Request(URL, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=30) as response:
        data = json.loads(response.read().decode("utf-8"))
    tmp_path = snapshot_path.with_suffix(snapshot_path.suffix + ".tmp")
    with tmp_path.open("w", encoding="utf-8") as handle:
        json.dump(data, handle, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        handle.write("\n")
    os.replace(tmp_path, snapshot_path)
    print(f"wrote {snapshot_path}")


if __name__ == "__main__":
    main()
