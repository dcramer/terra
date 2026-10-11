# Using our Tripo subscription from scripts

2026-10-11. Question: the bake-off's [tripo.py](../spike/model_bakeoff/tripo.py) spends API credits, which we find expensive, while we already pay for a Studio subscription. Has anyone found a way to drive generation from the subscription instead?

Mostly read, not tried: what follows comes from other people's code and READMEs on the day. We built a client from it, `tripo.py --studio` ([studio.py](../spike/model_bakeoff/studio.py)), and checked only what needs no login (end of note).

## Answer

Yes, but only through Tripo's private web backend. There is no supported route.

- **The two are billed separately.** Tripo's help centre says [web app credits are not API credits](https://www.tripo3d.ai/help/credit/how-do-credits-work), and the API has [its own balance](https://developers.tripo3d.com/en/docs/billing). Every official tool (the [MCP server](https://github.com/VAST-AI-Research/tripo-mcp), the Blender add-on, the [Codex plugin](https://developers.tripo3d.com/en/docs/codex-plugin)) uses an API key, and so does every community client we found except the three below.
- **Three projects call the backend that `studio.tripo3d.ai` uses**, with the login of a browser that is signed in:

| Project | What it covers | Last updated |
|---|---|---|
| [Lifecharger/auto-game-builder `tools/tripo`](https://github.com/Lifecharger/auto-game-builder/tree/main/tools/tripo) (GPL-3.0) | Python client for `api.tripo3d.ai/v2/studio/*`: text-to-model, progress, project detail, balance, rigging, retargeting. Takes the token from a Chrome attached over the DevTools protocol. Says Studio credits are "cheaper per-model than the API tier" | 2026-10-05; endpoints re-captured 2026-08-11 |
| [Zhengyuuuui/tripo2api](https://github.com/Zhengyuuuui/tripo2api) (no licence, in Chinese) | Local proxy and the fullest map: 115 endpoints, image upload, image-to-model, token renewal, accepted model versions | 2026-10-05 |
| [Sharks820/veilbreakers-gamedev-toolkit `tripo_studio_client.py`](https://github.com/Sharks820/veilbreakers-gamedev-toolkit/blob/main/Tools/mcp-toolkit/src/veilbreakers_mcp/shared/tripo_studio_client.py) | Older client on `/v2/web/task`, a path the others have moved away from | 2026-09-24 |

## How it works, as they describe it

- **Login.** Tripo uses Ory Kratos: an `ory_kratos_session` cookie that lasts about 30 days and cannot be renewed, and a short-lived bearer token (reported as 15 minutes to 2 hours) renewed from it. The cookie is captured once from a signed-in browser.
- **Cloudflare guards the website, not the API.** tripo2api reports that `api.tripo3d.ai` answers plain HTTP once you hold a token.
- **Model versions are dated IDs**, such as `v3.1-20260211`. The names in the app (P1, H3.1) are rejected.
- **Downloads are meshopt-compressed `.glb`**, which Blender reads.
- **Cost.** tripo2api measured 55 credits for an image-to-model. Lifecharger quotes 3,000 credits a month on the Professional plan.

## Limits

- **It breaks.** Lifecharger's original text-to-model endpoint now returns 410 Gone, and veilbreakers' paths are out of date. Expect to re-capture requests every few months.
- **Multiview is not covered.** That is the four-views-to-model step the bake-off uses. It would have to be captured from the app.
- **It is not a sanctioned use of the subscription.** Tripo could flag or close the account. Whether to accept that is the developers' call.
- **What we checked without a login:** the token endpoint (`/v2/studio/studio/whoami`) answers plain HTTP with a 401 for a made-up cookie, so it exists and Cloudflare does not stand in the way; and our hand-written S3 upload signature matches botocore's for the same inputs. Nothing has been generated yet, and the credit figures are other people's.
- **Which Studio setting gives P1's low poly is a guess.** tripo2api lists `smart_poly` as the P series; the app's own names (P1, H3.1) are rejected as model versions.
