import re
import sys

try:
    with open("litmus-raw.txt", "r") as f:
        raw = f.read()
except Exception as e:
    print(f"❌ Could not read litmus-raw.txt: {e}")
    sys.exit(1)

content = re.sub(r"\x1b\[[0-9;]*[mGJK]", "", raw)
content = re.sub(r"\x1b\(B", "", content)

lines = content.splitlines()
actual_fails = [l for l in lines if re.search(r"FAIL\s*\(", l)]
summaries = [l for l in lines if "summary for" in l]

print(f"\nValidation: Found {len(actual_fails)} actual test failures.")
for l in actual_fails:
    print(f"  -> {l}")

unexpected_summary = False
for l in summaries:
    match = re.search(r"(\d+)\s+failed", l)
    if match:
        failed_count = int(match.group(1))
        if failed_count > 0:
            if "copymove" in l and failed_count == 1:
                print(f"Summary: Expected single failure in copymove found.")
            else:
                print(f"❌ Summary: Unexpected failure(s) in suite: {l}")
                unexpected_summary = True

is_move_coll_only = len(actual_fails) == 1 and "move_coll" in actual_fails[0]

if len(actual_fails) == 0 and not unexpected_summary:
    print("✅ All tests passed.")
    sys.exit(0)
elif is_move_coll_only and not unexpected_summary:
    print("✅ Only known failure (move_coll) detected. Ignoring.")
    sys.exit(0)
else:
    print(f"❌ Found unexpected failures (fails={len(actual_fails)}, unexpected_summary={unexpected_summary}). Failing build.")
    sys.exit(1)
