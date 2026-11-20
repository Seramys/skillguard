import csv
import sys

with open(sys.argv[1], newline="") as handle:
    rows = list(csv.reader(handle))
print(f"rows={len(rows)} columns={len(rows[0]) if rows else 0}")
