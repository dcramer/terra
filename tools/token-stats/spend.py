"""Records credits spent on a service such as Tripo, for report.mjs to send with the tokens.

    from spend import record
    record("tripo-api", task_id, credits)

Or by hand, for something done outside our scripts, such as in Tripo's web app:

    python3 tools/token-stats/spend.py tripo-studio <task or project id> <credits>

Each task is one line of spends.jsonl in the state folder, which report.mjs reads. A task
recorded twice counts once, by its id, so recording again is harmless; deleting the file
loses what has not been sent yet. Standard library only. See README.md.
"""

import datetime
import json
import os
import re
import sys


def ledger():
    state = os.environ.get("XDG_STATE_HOME") or os.path.join(os.path.expanduser("~"), ".local", "state")
    return os.path.join(state, "terra-token-stats", "spends.jsonl")


def record(service, task, credits):
    """Notes that `task` on `service` took `credits`. Never fails the work it records."""
    # A balance may come back as 35.0; credits are whole.
    if isinstance(credits, float) and credits.is_integer():
        credits = int(credits)
    if not isinstance(credits, int) or isinstance(credits, bool) or credits < 0:
        print(f"spend: not recording {service} {task}: credits {credits!r} unknown", file=sys.stderr)
        return
    if credits == 0:
        return
    if not re.fullmatch(r"[a-z0-9-]{1,40}", service):
        print(f"spend: not recording {task}: service {service!r} is not lower-case letters, digits and -", file=sys.stderr)
        return
    line = {
        "id": re.sub(r"[^A-Za-z0-9_-]", "-", f"{service}-{task}")[:100],
        "day": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d"),
        "service": service,
        "credits": credits,
    }
    try:
        os.makedirs(os.path.dirname(ledger()), exist_ok=True)
        with open(ledger(), "a") as f:
            f.write(json.dumps(line) + "\n")
    except OSError as e:
        print(f"spend: could not record {line}: {e}", file=sys.stderr)


if __name__ == "__main__":
    if len(sys.argv) != 4 or not sys.argv[3].isdigit():
        sys.exit("usage: spend.py SERVICE TASK CREDITS")
    record(sys.argv[1], sys.argv[2], int(sys.argv[3]))
