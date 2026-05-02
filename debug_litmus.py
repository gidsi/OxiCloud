import re
import sys

with open("litmus-raw.txt", "r") as f:
    raw = f.read()

# Strip ANSI escape codes
content = re.sub(r"\x1b\[[0-9;]*[mGJK]", "", raw)

# Use re.IGNORECASE just in case
fails = len(re.findall(r" FAIL \(", content, re.IGNORECASE))
move_coll_fails = len(re.findall(r"move_coll.*FAIL", content, re.IGNORECASE))

print(f"Validation: Total FAILS={fails}, move_coll FAILS={move_coll_fails}")

# Check if there are other FAILS that are not move_coll
# Litmus uses FAIL (reason)
all_fail_lines = [line for line in content.splitlines() if " FAIL (" in line]
print(f"FAIL lines: {all_fail_lines}")
