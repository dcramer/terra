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


def key():
    if os.environ.get("TRIPO_API_KEY"):
        return os.environ["TRIPO_API_KEY"]
    root = os.path.abspath(os.path.join(HERE, "..", ".."))
    candidates = [os.path.join(root, ".env")]
    # A worktree's main checkout is three levels up from .claude/worktrees/<name>.
    if os.sep + os.path.join(".claude", "worktrees") + os.sep in root + os.sep:
        candidates.append(os.path.join(root.split(os.sep + ".claude" + os.sep)[0], ".env"))
    for path in candidates:
        if os.path.exists(path):
            for line in open(path):
                if line.startswith("TRIPO_API_KEY="):
                    return line.split("=", 1)[1].strip().strip('"')
    sys.exit("TRIPO_API_KEY is not set")


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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("kind", choices=["balance", "concept", "views", "multiview", "image", "text"])
    ap.add_argument("name", nargs="?")
    ap.add_argument("subject", nargs="*", help="the prompt, or the image paths")
    ap.add_argument("--image-model", default="chat_image_2")
    ap.add_argument("--quality", default="medium")
    ap.add_argument("--ref", action="append", default=[], help="a reference image for the concept")
    ap.add_argument("--model", default="P1-20260311")
    ap.add_argument("--faces", type=int)
    ap.add_argument("--parts", action="store_true", help="split into parts (H series only)")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--texture", action="store_true", help="a base-colour texture, to read each face's colour from")
    a = ap.parse_args()
    if a.kind == "balance":
        print(call("GET", "/account/balance"))
        return
    subject = " ".join(a.subject)
    if a.kind == "concept":
        request = {"model": a.image_model, "prompt": CONCEPT.format(subject=subject)}
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
