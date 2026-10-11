#!/usr/bin/env python3
"""A small client for Tripo's v3 API: make a model from text or an image, wait, download it.

    python3 tripo.py balance
    python3 tripo.py concept NAME "subject" [--image-model chat_image_2] [--ref img.png ...]
    python3 tripo.py views NAME concept.png          # four views from one image
    python3 tripo.py multiview NAME front.png left.png back.png right.png [--model ...]
    python3 tripo.py image NAME image.png [--model P1-20260311] [--faces 2000] [--parts]
    python3 tripo.py text NAME "subject" [--model ...] [--faces ...] [--parts]

The image first, then the model: a concept in the house style, checked by eye, then four
views of it, then a model from those. Writes out/tripo/NAME.* with NAME.json recording the
request, the task and the credits spent, so every model says where it came from. The key is
TRIPO_API_KEY, from the environment or a .env in this checkout or the main one.

--studio spends the Studio subscription's credits instead, through the web app's own backend
(studio.py; balance, concept, image and text only). It needs TRIPO_STUDIO_COOKIE the same way.
Standard library only.
"""

import argparse
import json
import os
import sys
import time
import urllib.request
import uuid

BASE = "https://openapi.tripo3d.ai/v3"
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "out", "tripo")
# Credits spent are recorded for the token stats (tools/token-stats).
sys.path.insert(0, os.path.join(HERE, "..", "..", "tools", "token-stats"))
from spend import record  # noqa: E402
# Every prompt gets the house style, so the generator aims at our look.
STYLE = "Low-poly stylised game asset, simple finely cut faceted shapes, flat planes, clean silhouette, no fine surface detail."
# The concept image's brief: one subject, readable, in our look.
CONCEPT = (
    "Concept art of a single {subject}, for a stylised low-poly game. Three-quarter view, the "
    "whole object in frame and centred, on a plain light grey background. Simple, finely cut "
    "faceted shapes made of flat planes, like a carefully lit papercraft model; flat colours, "
    "one colour per part, from a small soft palette; no textures, no outlines, no text; soft "
    "even light, no ground, no cast shadow."
)
NEGATIVE = "realistic, photoreal, noisy, fine detail, sculpted texture, base, pedestal, ground plane"


def env(name):
    if os.environ.get(name):
        return os.environ[name]
    root = os.path.abspath(os.path.join(HERE, "..", ".."))
    candidates = [os.path.join(root, ".env")]
    # A worktree's main checkout is three levels up from .claude/worktrees/<name>.
    if os.sep + os.path.join(".claude", "worktrees") + os.sep in root + os.sep:
        candidates.append(os.path.join(root.split(os.sep + ".claude" + os.sep)[0], ".env"))
    for path in candidates:
        if os.path.exists(path):
            for line in open(path):
                if line.startswith(name + "="):
                    return line.split("=", 1)[1].strip().strip('"').strip("'")
    sys.exit(f"{name} is not set")


def key():
    return env("TRIPO_API_KEY")


def call(method, path, body=None, raw=None, content_type="application/json"):
    data = raw if raw is not None else (json.dumps(body).encode() if body is not None else None)
    req = urllib.request.Request(BASE + path, data=data, method=method)
    req.add_header("Authorization", f"Bearer {key()}")
    if data is not None:
        req.add_header("Content-Type", content_type)
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            reply = json.load(r)
    except urllib.error.HTTPError as e:
        reply = json.loads(e.read() or b"{}")
    if reply.get("code") != 0:
        sys.exit(f"{method} {path}: {reply}")
    return reply["data"]


def inputs(paths):
    return [upload(p) for p in paths]


def upload(path):
    boundary = uuid.uuid4().hex
    name = os.path.basename(path)
    body = (
        f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="{name}"\r\n'
        f"Content-Type: image/png\r\n\r\n"
    ).encode() + open(path, "rb").read() + f"\r\n--{boundary}--\r\n".encode()
    data = call("POST", "/files", raw=body, content_type=f"multipart/form-data; boundary={boundary}")
    return data.get("file_token") or data.get("token")


def wait(task):
    while True:
        data = call("GET", f"/tasks/{task}")
        status = data.get("status")
        if status == "success":
            return data
        if status in ("failed", "cancelled", "banned", "expired"):
            sys.exit(f"task {task} {status}: {data}")
        print(f"  {task}: {status} {data.get('progress', '')}%", file=sys.stderr)
        time.sleep(5)


def fetch(url, path):
    with urllib.request.urlopen(url, timeout=300) as r, open(path, "wb") as f:
        f.write(r.read())


def generate(name, endpoint, request):
    os.makedirs(OUT, exist_ok=True)
    task = call("POST", endpoint, request)["task_id"]
    print(f"{name}: task {task}", file=sys.stderr)
    result = wait(task)
    record("tripo-api", task, result.get("credits_consumed"))
    files = save(name, result)
    record = {"name": name, "endpoint": endpoint, "request": request, "task": task, "result": result, "files": files}
    with open(os.path.join(OUT, f"{name}.json"), "w") as f:
        json.dump(record, f, indent=2)
    print(json.dumps({"name": name, "credits": result.get("credits_consumed"), "files": files}))


def flatten(output):
    """Every URL in a task's output, however deeply it is nested."""
    for field, value in output.items():
        if isinstance(value, dict):
            yield from flatten(value)
        else:
            yield field, value


def save(name, result):
    """Downloads every file in a task's result next to its record."""
    files = {}
    for field, url in flatten(result.get("output", {})):
        if isinstance(url, str) and url.startswith("http"):
            ext = ".glb" if "model" in field else os.path.splitext(url.split("?")[0])[1] or ".bin"
            path = os.path.join(OUT, f"{name}{'' if field == 'model_url' else '-' + field}{ext}")
            fetch(url, path)
            files[field] = os.path.relpath(path, HERE)
    return files


def studio(a, subject):
    """The same models from the Studio subscription's credits (studio.py). Image and text only so far."""
    if a.kind not in ("balance", "concept", "image", "text") or (a.kind == "concept" and a.ref):
        sys.exit(f"{a.kind}{' with --ref' if a.ref else ''} is not captured from the Studio app yet; use the API, without --studio")
    sys.path.insert(0, HERE)  # python3 -I leaves this folder off the path
    from studio import Studio, node_name

    client = Studio(env("TRIPO_STUDIO_COOKIE"))
    if a.kind == "balance":
        print(client.call("GET", "/v2/studio/user/profile/payment").get("wallet"))
        return
    if a.kind == "concept":
        os.makedirs(OUT, exist_ok=True)
        request = {
            "if_upload": True,
            "sketch_to_render": False,
            "t_pose": False,
            "amount": 1,
            "model_version": "gpt_image_2.5_sunburst",
            "prompt": subject if a.raw else CONCEPT.format(subject=subject),
            "resolution": "1K",
            "scale": "1:1",
        }
        before = client.credits()
        started, asset = client.image(request)
        path = os.path.join(OUT, f"{a.name}-generated_image_url.png")  # the API's name, for make.sh
        fetch(asset["output"]["data"][0]["url"], path)
        out = {"name": a.name, "backend": "studio", "request": request, "started": started, "credits": before - client.credits(), "result": asset, "files": {"generated_image_url": os.path.relpath(path, HERE)}}
        record("tripo-studio", started["asset_id"], out["credits"])
        with open(os.path.join(OUT, f"{a.name}.json"), "w") as f:
            json.dump(out, f, indent=2)
        print(json.dumps({"name": a.name, "credits": out["credits"], "files": out["files"]}))
        return
    model = a.model or "v3.1-20260211"
    # Smart Mesh (P1, run as Nexus) rejects texture settings: it textures in a second step.
    smart_mesh = model.startswith(("P", "Nexus"))
    request = {
        "model_version": model,
        "geometry_quality": "standard",
        "texture": a.texture and not smart_mesh,
        "pbr": False,
        "generate_parts": a.parts,
        "smart_poly": a.smart_poly,
        "quad": False,
        "face_limit": a.faces or 10000,
        "visibility": "private",
    }
    if request["texture"]:
        request["texture_quality"] = "standard"  # the app sends it with every texture
    if a.kind == "text":
        request.update(prompt=f"{subject} {STYLE}", t_pose=False, sketch_to_render=False)
        endpoint = "/v2/studio/operation/text_to_model"
    else:
        request.update(image=client.upload(a.subject[0]), texture_alignment="original_image", delight=False, enable_image_autofix=False)
        endpoint = "/v2/studio/operation/image_to_model"
    os.makedirs(OUT, exist_ok=True)
    started, project, credits = client.generate(endpoint, request)
    path = os.path.join(OUT, f"{a.name}.glb")
    fetch(project["model_url"], path)
    textured = None
    if a.texture and smart_mesh and "image" in request:
        print("Smart Mesh comes untextured; texturing it", file=sys.stderr)
        textured, project, more = client.texture(started["project_id"], request["image"], node_name(path))
        credits += more
        fetch(project["model_url"], path)
    files = {"model_url": os.path.relpath(path, HERE)}
    record = {"name": a.name, "backend": "studio", "endpoint": endpoint, "request": request, "started": started, "textured": textured, "credits": credits, "project": project, "files": files}
    with open(os.path.join(OUT, f"{a.name}.json"), "w") as f:
        json.dump(record, f, indent=2)
    print(json.dumps({"name": a.name, "credits": credits, "files": files}))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("kind", choices=["balance", "concept", "views", "multiview", "image", "text"])
    ap.add_argument("name", nargs="?")
    ap.add_argument("subject", nargs="*", help="the prompt, or the image paths")
    ap.add_argument("--image-model", default="chat_image_2")
    ap.add_argument("--quality", default="medium")
    ap.add_argument("--ref", action="append", default=[], help="a reference image for the concept")
    ap.add_argument("--raw", action="store_true", help="use the subject as the whole prompt, without the house brief")
    ap.add_argument("--model", help="P1-20260311 on the API; a dated version such as v3.1-20260211 on Studio")
    ap.add_argument("--faces", type=int)
    ap.add_argument("--parts", action="store_true", help="split into parts (H series only)")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--texture", action="store_true", help="a base-colour texture, to read each face's colour from")
    ap.add_argument("--studio", action="store_true", help="spend the Studio subscription's credits, not the API's")
    ap.add_argument("--smart-poly", action="store_true", help="Studio's low-poly mesh (thought to be the P series)")
    a = ap.parse_args()
    if a.studio:
        studio(a, " ".join(a.subject))
        return
    a.model = a.model or "P1-20260311"
    if a.kind == "balance":
        print(call("GET", "/account/balance"))
        return
    subject = " ".join(a.subject)
    if a.kind == "concept":
        request = {"model": a.image_model, "prompt": subject if a.raw else CONCEPT.format(subject=subject)}
        if a.image_model.startswith("chat_image_2"):
            request["quality"] = a.quality
        if a.ref:
            refs = inputs(a.ref)
            request.update(input=refs[0], inputs=refs)
            generate(a.name, "/generation/image-to-image", request)
        else:
            generate(a.name, "/generation/text-to-image", request)
        return
    if a.kind == "views":
        generate(a.name, "/generation/image-to-multiview", {"input": upload(a.subject[0])})
        return
    request = {"model": a.model, "texture": a.texture, "pbr": False, "model_seed": a.seed}
    if a.faces:
        request["face_limit"] = a.faces
    if a.parts:
        request["generate_parts"] = True
    if a.kind == "text":
        request.update(prompt=f"{subject} {STYLE}", negative_prompt=NEGATIVE, image_seed=a.seed)
        generate(a.name, "/generation/text-to-model", request)
    elif a.kind == "multiview":
        views = ["front", "left", "back", "right"]
        request["inputs"] = [{v: t} for v, t in zip(views, inputs(a.subject)) if t]
        generate(a.name, "/generation/multiview-to-model", request)
    else:
        request["input"] = upload(a.subject[0])
        generate(a.name, "/generation/image-to-model", request)


if __name__ == "__main__":
    main()
