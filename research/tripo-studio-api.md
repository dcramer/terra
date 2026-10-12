# Tripo Studio's API, endpoint by endpoint

2026-10-11. Question: what does each operation of Studio's private backend take and return, so a script can do everything the web app does on our subscription's credits? Follows [tripo-studio-credits.md](tripo-studio-credits.md), which found the backend; the result is the team's CLI, [tools/tripo](../tools/tripo).

## How it was found

- **Captured from the web app.** A browser signed in as us drove the app, image to views to model, then rig, animate and export, recording every request and response (a HAR file). The browser has to be visible, since Cloudflare stops a headless one with a challenge for a person to pass, and needs WebGL (software SwiftShader is enough), or the 3D view stays empty and the rig and animate buttons stay disabled. Hundreds of progress polls were boiled down to one request and reply per endpoint, so the capture stays readable.
- **Read from the app's JavaScript.** The production bundle (185 chunks) names every endpoint the app calls and builds each request body in the open, so the bodies of operations we did not run could be read rather than guessed.
- **Checked against other people's clients**: [Lifecharger/auto-game-builder](https://github.com/Lifecharger/auto-game-builder), [RainNameless/aigccat](https://github.com/RainNameless/aigccat), [Kydaix/Tripo-MCP](https://github.com/Kydaix/Tripo-MCP), [GrinZero/tripo-studio-plugin](https://github.com/GrinZero/tripo-studio-plugin) and [Zhengyuuuui/tripo2api](https://github.com/Zhengyuuuui/tripo2api). Where they disagreed, what the app sends today won.
- **Run.** Every CLI command was run once against the live service on 2026-10-11, and its output opened.

## Answer

Everything the app does to a model is one request to `POST /v2/studio/operation/<name>`, answered with `{project_id, operator_id}`. `POST /v2/studio/progress {"ids": [operator_id]}` says when it is done, and `GET /v2/studio/project/detail/v3/<project>?operator_id=<op>` gives that result's `model_url`. A project keeps every operation done to it, and each new one works on its latest. Images are their own family under `/v2/studio/image/`, polled with `get_image_asset`.

| Operation | Request (beyond `project_id`) | Evidence | Credits seen |
|---|---|---|---|
| Image | `image/gen_image_v2 {prompt, model_version, amount, scale, resolution, t_pose, if_upload, sketch_to_render}`; one reference in `image`, two to ten in `images` | captured, run | free (100 a month on Professional) |
| Views | `image/gen_multiview {if_upload, image}`; four outputs, **front, left, back, right**. The app shows them as front, left, right, back, but sends and stores them in that order | captured, run | 20 in the app, 0 from the CLI |
| Model from an image | `image_to_model {model_version, image, face_limit, quad, visibility, texture, …}` | captured earlier, run | 25 (H3.1, textured) |
| Model from views | `multiview_to_model`, the same with `image` a list of four, `null` for a missing view | captured, run | 25 (H3.1); 55 detailed with PBR |
| Model from text | `text_to_model {prompt, gen_image_model_version: "flux.1_dev", t_pose, sketch_to_render, …}` | bundle, run | 25 |
| Smart Mesh (P1, P2) | the same with `model_version` `Nexus-v1.0-20260214` or `Nexus-v2.0-20260801` and none of the texture fields, which it refuses even when false; P2 adds `symmetry` from the free `symmetry_check` | bundle, run | 35, then 10 to texture |
| Texture | `texture_model {part_names, texture_quality, texture_alignment, delight}` and one of `image`, `images` or `prompt_text` | bundle, run | 10 |
| Texture to 4K or 8K | `texture_upscaler {model_version: "v3.0-20250812", texture_quality: detailed\|extreme}` | bundle, run | 10 |
| PBR maps | `pbr_generate {model_version: "v3.0-20250812"}` | bundle, run | 5 |
| Retopology | `remesh {face_limit, quad, smart_poly, bake: true, model_version: "default", part_name_list}` | bundle, run | 5 |
| Segment | `ai_segmentation {model_version: "v2.0-20260430", segmentation_granularity: simple\|balanced\|detailed}` | bundle, run | 40 |
| Fill holes | `mesh_fill` or `ai_completion` with `part_names` | other clients, run | |
| Smart UV | `uv_edit/context`, then `uv_edit/generate {current_operator_id, action}`, then `uv_edit/apply {candidate_operator_id}` | bundle, run | 0 on a trial; 20 listed |
| Rig | `pre_rig_check {model_version}` (free) gives `rig_type`; then `rigging_model {model_version, rig_type, spec}` | captured, run | 20 |
| Animate | `retarget_model {animations: ["preset:<rig_type>:<name>"], rig_type, model_version: "default"}`, or `motion_asset_id` for a motion made with `motion/generate` | captured, run | presets free |
| Export | `export {format, animations: [retarget operator ids], with_animation, texture_size, texture_packaging, fbx_preset, …}`, then `download_with_name` for the file | captured, run | free |
| Import | `import_user_model {format, model: {bucket, key}, name, transform_matrix, use_original_uv}` | bundle, run | free |
| Spending | `GET txn/records {page_num, page_size}`: every charge with its operation, type, credits and status (`pending`, `fulfilled`, `cancelled`); one operation can be billed as two records | run | |

Things the table does not say:

- **Two rigs.** The app's Auto Rig sends `v3.0-20260909` with a `spec` (`mixamo`, `actorcore`, `unreal`, `unity` or `vrm`): ours came back with 65 joints named `mixamorig:*`, fingers included. The older `v1.0-20240301`, which the other clients and our skeleton converter use, now gives Tripo's own naming (`BoneRoot`, `Hip`, `Pelvis`, `L_Thigh`…) with 78 joints, fingers included; other people's clients reported 41 joints and no fingers from it a month earlier, so it has changed since. Rigged on our base body on 2026-10-12, by preset: only V3's `unreal` puts skin on its twist bones (`upperarm_twist_01`, `lowerarm_twist_01`, `thigh_twist_01` and `calf_twist_01` per side, 300–2,300 vertices each), though it has no share bones; V1 and V3's `actorcore` have twist and share bones on every limb that move no vertex at all; `mixamo` and `unity` have no limb twist bones (`unity` has one at the neck).
- **One operation at a time per project.** Studio refuses a new one while another runs ("the status of task is not success", 403) and reports the running one as `running_operator` in the project's detail. A texture from a prompt took over 15 minutes.
- **Progress polls fail now and then** (one 502 in a few hundred); reading again is safe, starting an operation again is not, since it could pay twice.
- **Animations go one at a time.** A project takes one retarget at a time and answers 406 to another while one runs. Each lands in the project's `operator.retarget` list with its own `model_url`, and `export` puts several in one file.
- **Export formats.** GLB is `format: "gltf"` on the wire. With the app's `texture_packaging: "zip"`, a GLB still comes back as one file but an FBX comes back as a zip; `embedded` gives one FBX. `model_url` itself is a meshopt-compressed GLB, which some tools cannot read; an export is not.
- **Uploads** go to S3 with temporary keys from `storage/temporary_token`, then images through `audit/image`. The app signs its PUT to the accelerated endpoint in headers; a presigned PUT to the regional endpoint works as well.
- **Credits** are read from the wallet before and after, so they are approximate when two runs overlap. Views cost 20 when made in the app and 0 when made by the CLI, a difference we cannot explain from here.

## Capturing again

Tripo will change these requests. To see what the app sends now: open a visible browser with WebGL (with agent-browser: `--headed --args "--use-angle=swiftshader,--enable-unsafe-swiftshader"`), set the `ory_kratos_session` cookie on `.tripo3d.ai`, start a HAR recording, do the thing in the app once, and save the HAR outside the repository. A HAR holds live tokens: never commit one. Keep one example per endpoint and drop the progress polls before reading it.

## Limits

- Bodies read from the bundle are what the app sends today; Tripo can change them without notice.
- Most costs come from one run each, read off the wallet.
- Not covered: team and billing endpoints, the gallery, Magic Brush (it needs a render of the viewport), image upscale and cut-out of an existing image, local edits.
