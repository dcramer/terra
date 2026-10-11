"""Tripo Studio's own backend, so a model spends the subscription's credits, not the API's.

Not a public API: the requests are the ones studio.tripo3d.ai makes, as worked out by
Lifecharger/auto-game-builder and Zhengyuuuui/tripo2api (research/tripo-studio-credits.md).
Expect it to break when Tripo changes the site.

The login is TRIPO_STUDIO_COOKIE: the whole `cookie` request header of any call the signed-in
web app makes to api.tripo3d.ai (developer tools, Network). It holds the ory_kratos_session,
which lasts about 30 days; each run trades it for a bearer token that lasts 15 minutes.
"""

import base64
import datetime
import hashlib
import hmac
import json
import sys
import time
import urllib.parse
import urllib.request

API = "https://api.tripo3d.ai"
BROWSER = {
    "Accept": "application/json",
    "Origin": "https://studio.tripo3d.ai",
    "Referer": "https://studio.tripo3d.ai/",
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36",
}


class Studio:
    def __init__(self, cookie):
        self.cookie = cookie.strip()
        jar = dict(p.strip().split("=", 1) for p in self.cookie.split(";") if "=" in p)
        if "ory_kratos_session" not in jar:
            sys.exit("TRIPO_STUDIO_COOKIE has no ory_kratos_session: copy the cookie header again")
        self.device = jar.get("tripo_device_id", "")
        self.csrf = next((v for k, v in jar.items() if k.startswith("csrf_token_")), "")
        self.jwt, self.expires = "", 0.0

    def send(self, method, path, body=None, jwt=True):
        req = urllib.request.Request(API + path, method=method, headers=dict(BROWSER, Cookie=self.cookie))
        if jwt:
            req.add_header("Authorization", f"Bearer {self.token()}")
        if self.device:
            req.add_header("x-tripo-device-id", self.device)
        if self.csrf:
            req.add_header("csrf_token", self.csrf)
        req.add_header("x-tripo-start", f"{time.perf_counter() * 1000:.1f}")
        if body is not None:
            req.data = json.dumps(body).encode()
            req.add_header("Content-Type", "application/json")
        try:
            with urllib.request.urlopen(req, timeout=120) as r:
                return r.status, json.load(r)
        except urllib.error.HTTPError as e:
            return e.code, json.loads(e.read() or b"{}")

    def token(self):
        """A bearer token from the session cookie, renewed shortly before it runs out."""
        if time.time() < self.expires - 120:
            return self.jwt
        status, reply = self.send("GET", "/v2/studio/studio/whoami?tokenizeAs=default_jwt", jwt=False)
        if status != 200 or not reply.get("tokenized"):
            sys.exit(f"Studio login failed ({status}): the session cookie has probably expired; copy it again")
        self.jwt = reply["tokenized"]
        payload = self.jwt.split(".")[1]
        self.expires = json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))["exp"]
        return self.jwt

    def call(self, method, path, body=None):
        status, reply = self.send(method, path, body)
        if reply.get("code") != 0:
            sys.exit(f"{method} {path} ({status}): {reply}")
        return reply.get("data", {})

    def credits(self):
        return self.call("GET", "/v2/studio/user/profile/payment").get("wallet", {}).get("total_credit")

    def upload(self, path):
        """Puts an image in Tripo's bucket and has it checked, as the web app does before a model."""
        ext = path.rsplit(".", 1)[-1].lower().replace("jpg", "jpeg")
        sts = self.call("POST", "/v2/studio/storage/temporary_token", {"client": "aws", "format": ext})
        bucket, key = sts["resource_bucket"], sts["resource_uri"]
        region = sts.get("host", "").split("s3.")[-1].split(".amazonaws")[0] or "us-west-2"
        url = presign_put(bucket, key, region, sts["sts_ak"], sts["sts_sk"], sts.get("session_token"))
        req = urllib.request.Request(url, data=open(path, "rb").read(), method="PUT")
        req.add_header("Content-Type", f"image/{ext}")
        urllib.request.urlopen(req, timeout=120).close()
        audit = self.call("POST", "/v2/studio/audit/image", {"image": {"bucket": bucket, "key": key}})
        return {"bucket": bucket, "key": key, "image_source": "upload", "image_audit_result": audit.get("result", "pass")}

    def wait(self, operator):
        """Until the operation says success. Progress sits at 99% for a minute or two while it bakes."""
        while True:
            item = (self.call("POST", "/v2/studio/progress", {"ids": [operator]}) or [{}])[0]
            status = item.get("status")
            if status == "success":
                return item
            if status in ("failed", "error", "cancelled"):
                sys.exit(f"operation {operator} {status}: {item}")
            print(f"  {operator}: {status} {item.get('progress', '')}%", file=sys.stderr)
            time.sleep(5)

    def generate(self, endpoint, request):
        """Runs one operation and returns its finished project, with the credits it took."""
        before = self.credits()
        started = self.call("POST", endpoint, request)
        project_id = started.get("project_id") or request["project_id"]
        print(f"project {project_id}, operation {started.get('operator_id')}", file=sys.stderr)
        self.wait(started["operator_id"])
        project = self.call("GET", f"/v2/studio/project/detail/v3/{project_id}")
        return started, project, before - self.credits()

    def texture(self, project_id, image, part):
        """Textures a finished model from its image, as the app's Texture tool does. Smart Mesh
        models (P1) come back untextured; the API does this in the same call. The part is the
        model's node name (node_name), or base_model_path for an HD model."""
        request = {
            "project_id": project_id,
            "image": {"bucket": image["bucket"], "key": image["key"]},
            "part_names": [part],
            "texture_alignment": "original_image",
            "texture_quality": "standard",
            "delight": False,
        }
        return self.generate("/v2/studio/operation/texture_model", request)


def node_name(glb_path):
    """The first node's name in a .glb: tripo_node_<operation>, the part the texture step wants."""
    with open(glb_path, "rb") as f:
        head = f.read(20)
        chunk = json.loads(f.read(int.from_bytes(head[12:16], "little")))
    return chunk["nodes"][0]["name"]


def presign_put(bucket, key, region, ak, sk, session_token, expires=900):
    """An S3 presigned PUT URL (SigV4), from the temporary credentials the upload token gives."""
    now = datetime.datetime.now(datetime.timezone.utc)
    stamp, day = now.strftime("%Y%m%dT%H%M%SZ"), now.strftime("%Y%m%d")
    scope = f"{day}/{region}/s3/aws4_request"
    host = f"{bucket}.s3.{region}.amazonaws.com"
    uri = "/" + urllib.parse.quote(key.lstrip("/"), safe="/~")
    query = {
        "X-Amz-Algorithm": "AWS4-HMAC-SHA256",
        "X-Amz-Credential": f"{ak}/{scope}",
        "X-Amz-Date": stamp,
        "X-Amz-Expires": str(expires),
        "X-Amz-SignedHeaders": "host",
    }
    if session_token:
        query["X-Amz-Security-Token"] = session_token
    qs = "&".join(f"{k}={urllib.parse.quote(query[k], safe='')}" for k in sorted(query))
    canonical = "\n".join(["PUT", uri, qs, f"host:{host}\n", "host", "UNSIGNED-PAYLOAD"])
    to_sign = "\n".join(["AWS4-HMAC-SHA256", stamp, scope, hashlib.sha256(canonical.encode()).hexdigest()])
    k = ("AWS4" + sk).encode()
    for part in (day, region, "s3", "aws4_request"):
        k = hmac.new(k, part.encode(), hashlib.sha256).digest()
    signature = hmac.new(k, to_sign.encode(), hashlib.sha256).hexdigest()
    return f"https://{host}{uri}?{qs}&X-Amz-Signature={signature}"
