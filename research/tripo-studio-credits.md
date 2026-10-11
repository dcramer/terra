# Using our Tripo subscription from scripts

2026-10-11. Question: the bake-off's [tripo.py](../spike/model_bakeoff/tripo.py) spends API credits, which we find expensive, while we already pay for a Studio subscription. Has anyone found a way to drive generation from the subscription instead?

What follows comes from other people's code and READMEs on the day. We built a client from it, `tripo.py --studio` ([studio.py](../spike/model_bakeoff/studio.py)), and made one model with it (end of note).

## Answer

Yes, but only through Tripo's private web backend. There is no supported route. We tried it, and it works.

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
- **Which Studio setting gives P1's low poly is still a guess.** tripo2api lists `smart_poly` as the P series; the app's own names (P1, H3.1) are rejected as model versions.

## Tried

The same day, with the session cookie of our Professional account (3,200 credits). Our hand-written S3 upload signature was first checked against botocore's for the same inputs: they match.

The bake-off's well, from the concept image the API had already made a model of:

| | API (`prop-well-p1`) | Studio (`prop-well-studio`) |
|---|---|---|
| Request | image-to-model, P1-20260311, 2,500 faces, textured | image-to-model, v3.1-20260211, `smart_poly`, 2,500 faces, untextured |
| Credits | 50 API credits | 35 Studio credits |
| Triangles | 2,361 | 3,595 |
| File | 647 KB, with a texture | 23 KB, meshopt-compressed, no texture |
| Time | | About 2 minutes |

The whole path worked: login from the cookie, upload, generation, download. Side by side, the shapes are close: the same stone ring, posts, crossbeam and bucket. Studio's roof shingles are more distinct, its edges softer and less faceted than P1's, and it is half as many triangles again over the face limit, so `smart_poly` on v3.1 may not be P1. The run was untextured, so `refine.py` cannot colour it yet; a textured Studio run is the next comparison.
