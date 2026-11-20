import json
import subprocess
import urllib.parse

API = "https://clawhub.ai/api/v1"


def fetch(url):
    result = subprocess.run(
        ["curl", "-sS", "-m", "20", "--resolve", "clawhub.ai:443:216.150.1.1", url],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    return json.loads(result.stdout)


page = fetch(f"{API}/skills?limit=3")
cursor = page["nextCursor"]
print("page1", [item["slug"] for item in page["items"]])

raw = cursor if isinstance(cursor, str) else json.dumps(cursor, separators=(",", ":"))
page2 = fetch(f"{API}/skills?limit=3&cursor={urllib.parse.quote(raw, safe='')}")
print("page2", [item["slug"] for item in page2["items"]])