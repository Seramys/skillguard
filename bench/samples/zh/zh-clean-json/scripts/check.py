import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    data = json.load(handle)
print("ok", len(data))
