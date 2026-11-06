#!/usr/bin/env python3
"""Add Background App Refresh keys to a packaged Pulse Info.plist.

NSH 0.3.0 writes a fixed Info.plist. `nsh.json` cannot declare
UIBackgroundModes or BGTaskSchedulerPermittedIdentifiers, so run this after
`nsh package --target ios` and before Sideloadly:

    python3 scripts/ios-background-plist.py .nsh/out/Pulse.app/Info.plist

A signed ipa keeps the same file at Payload/Pulse.app/Info.plist. The script
is a no-op when the keys are already present. Sideload and ad-hoc builds can
still be denied a wake by iOS; the app logs that and keeps local notices for
while it is open.
"""

import plistlib
import sys

TASK_ID = "dev.pulse.app.refresh"


def enable(path: str) -> None:
    with open(path, "rb") as handle:
        data = plistlib.load(handle)
    modes = list(data.get("UIBackgroundModes") or [])
    if "fetch" not in modes:
        modes.append("fetch")
    data["UIBackgroundModes"] = modes
    identifiers = list(data.get("BGTaskSchedulerPermittedIdentifiers") or [])
    if TASK_ID not in identifiers:
        identifiers.append(TASK_ID)
    data["BGTaskSchedulerPermittedIdentifiers"] = identifiers
    with open(path, "wb") as handle:
        plistlib.dump(data, handle, fmt=plistlib.FMT_XML)


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(f"usage: {sys.argv[0]} Info.plist")
    enable(sys.argv[1])


if __name__ == "__main__":
    main()
