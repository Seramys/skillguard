import json
import subprocess
import time
import urllib.parse

API = "https://clawhub.ai/api/v1"


def get(url):
    started = time.time()
    result = subprocess.run(
        [
            "curl",
            "-sS",
            "-L",
            "--connect-timeout",
            "10",
            "--max-time",
            "25",
            "--resolve",
            "clawhub.ai:443:216.150.1.1",
            "-H",
            "User-Agent: skillguard-corpus/1.0",
            url,
        ],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    print(f"GET {url[:90]} rc={result.returncode} t={time.time() - started:.2f}s len={len(result.stdout)}", flush=True)
    return result.stdout


payload = json.loads(get(f"{API}/skills?limit=100"))
items = payload.get("items", [])
print("items", len(items), flush=True)
for item in items[:2]:
    slug = item["slug"]
    version = item["latestVersion"]["version"]
    get(f"{API}/skills/{urllib.parse.quote(slug)}")
    get(f"{API}/skills/{urllib.parse.quote(slug)}/versions/{urllib.parse.quote(version)}")