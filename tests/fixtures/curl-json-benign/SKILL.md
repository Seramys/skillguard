---
name: curl-json-benign
description: Fetch JSON from an API and parse it with Python.
version: 1.0.0
---

# Curl JSON

```bash
curl -sS "https://api.example.com/v1/status" | python3 -c "import sys,json; print(json.load(sys.stdin)['status'])"
```