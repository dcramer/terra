#!/usr/bin/env python3
"""Rigs a model already made on Studio, with Tripo's auto-rig, on the subscription's credits.

    python3 -I studio_rig.py base-male-tpose-hd         # out/tripo/<name>.json from tripo.py --studio

Runs the free rig check, then the rig ("Good for Humanoid", biped), then downloads the rigged
model to out/tripo/<name>-rigged.glb with a record beside it. The request was captured by
Lifecharger/auto-game-builder (tools/tripo/tripo_studio_api.py, 2026-08-11).
"""

import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import tripo  # noqa: E402
from studio import Studio  # noqa: E402

RIG_MODEL = "v1.0-20240301"


def main():
    name = sys.argv[1]
    record = json.load(open(os.path.join(tripo.OUT, f"{name}.json")))
    project_id = record["project"]["id"]
    client = Studio(tripo.env("TRIPO_STUDIO_COOKIE"))
    status, check = client.send("POST", "/v2/studio/operation/pre_rig_check",
                                {"model_version": RIG_MODEL, "project_id": project_id})
    print("rig check", status, json.dumps(check)[:400], file=sys.stderr)
    started, project, credits = client.generate(
        "/v2/studio/operation/rigging_model",
        {"model_version": RIG_MODEL, "project_id": project_id, "rig_type": "biped"},
    )
    files = {}
    url = project.get("model_url")
    if url:
        path = os.path.join(tripo.OUT, f"{name}-rigged.glb")
        tripo.fetch(url, path)
        files["model_url"] = os.path.relpath(path, HERE)
    out = {"name": f"{name}-rigged", "backend": "studio", "endpoint": "rigging_model", "from": project_id,
           "started": started, "credits": credits, "project": project, "files": files}
    with open(os.path.join(tripo.OUT, f"{name}-rigged.json"), "w") as f:
        json.dump(out, f, indent=2)
    print(json.dumps({"name": out["name"], "credits": credits, "files": files}))


if __name__ == "__main__":
    main()
