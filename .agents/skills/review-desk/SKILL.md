---
name: review-desk
description: Raise a question, a still or a film for the developer's review on their review desk (a claude.ai artifact page), read their answers, and turn settled design answers into decisions. Use when you need the developer's call or eye, when an agent hands you a FOR REVIEW line, when reading answers, or when setting up a desk the first time.
---

# Review desk

The desk (`tools/review-desk/`, D422) is where the developer answers questions and reviews stills and films, from any machine, so nothing waits in chat. Each developer has their own desk and their own coordinator; nothing is shared or centralised. The desk is personal triage; a settled design answer becomes a decision (D-file), which is the shared record. Only the coordinator (the session talking to the developer) touches the desk; subagents hand it items.

- **Never in chat**: a question or review for the developer goes on the desk, not into chat to scroll away. Chat says only "on the desk: <title>".
- **Everything on the desk**: the developer reads from another machine, so every still and film is uploaded as an asset and linked by the URL the upload returns, never a local path.
- **No personal data** in a title, body or caption; the page is private to the developer but the source is public.

## Set up your desk (once)

1. Read `~/.config/terra/desk.json`. If it has a `url`, that is your desk: skip to the next section.
2. Otherwise publish the page: the Artifact tool, `file_path: tools/review-desk/index.html`, `icon: "inbox"`, `capabilities: {"db": {}, "user": {}, "assets": {}}` (load the `artifact-capabilities` skill first). It stays private.
3. Save the URL it returns: `mkdir -p ~/.config/terra && echo '{"url":"<url>"}' > ~/.config/terra/desk.json`. This file is local and never committed. Tell the developer the link once.
4. After the page changes in the repo, republish with the same `file_path` and the `url` from the file; items and answers live in the database, not the page, so they survive.

## Add an item

Use the ArtifactData tool on the desk's `url`: `set` one document in `items` (`batch` for several). The shape is in [tools/review-desk/README.md](../../../tools/review-desk/README.md): `kind` (`question` or `review`), `area`, `status: "open"`, `createdAt` (ISO time), `title`, `body`, `ask`, `options: [{label, detail}]`, `images: [{url,caption}]`, `videos: [{url,caption}]`, and empty `answer` and `note`.

- **Every item carries `ask`**: one sentence saying exactly what the developer should decide or look at (shown as "What I need from you"). An item without it leaves the developer guessing.
- **A composite still's caption says where each panel is** ("top row: A, B, C; row 2: D, E"), so the developer can tell which is which; the page opens a still larger on click.

- **A question**: the title is the question, the body the context a cold reader needs, two to four options with your recommendation in the detail. A design question is also recorded as an Open decision; build nothing on it until answered (AGENTS.md, Decisions).
- **A review**: what to look at and what you want judged ("does the fire read as warm?"), with its stills and films.
- **A still**: a PNG or JPEG under 15 MB. Upload it with the Artifact tool: `asset: true`, `url` the desk's, `file_path` (or up to 25 in `file_paths`). The result gives each file's `url`; put it in `images` with a caption saying which shot.
- **A film**: re-encode, check it is under 15 MB (raise `-crf` or cut it if not), upload as an asset, put it in `videos` with a caption:

  ```bash
  ffmpeg -i in.mp4 -an -c:v libx264 -crf 28 -preset slow -vf scale=1600:-2 -movflags +faststart out.mp4
  ```

- Files that must outlive the session live in `~/.cache/terra-review`, never `/tmp` (tmpfs). Once uploaded, a file is on the desk and the local copy is disposable.

## Read answers (every turn)

1. ArtifactData `query` on `items` for `status: "answered"` whose `note` does not contain "Picked up:". Answers do not notify the session, so look each turn you coordinate; a hook (`tools/hooks/desk-answers.ts`) reminds you on each of the developer's messages once the session has used the desk.
2. Act on each answer, then `update` the item, appending `Picked up: <what you did, one line>` to its `note` after the developer's text, so the next turn skips it.
3. A review's note is the brief for the next job; a question's `answer` is the option label picked and its `note` the nuance.

## Remove stale items

An answered item picked up a day ago, a question overtaken, a review of a render since replaced: ArtifactData `delete` it, and its assets with the Artifact tool (`action: "delete"`, `url`, `path` the asset id) when nothing references them. The Open tab holds only what waits on the developer.

## A settled design answer becomes a decision

The desk is not the record. When an answer settles a design question (gameplay, art, scope), write the decision in the same turn: `cargo xtask decision` for the number, `docs/decisions/Dnnn.md` with `By: Developers`, Status Decided, the answer quoted in the Why ("The developers, <date>: \"<answer and note>\""), the log line, then `node tools/docs/docs.ts --render` (AGENTS.md, Decisions). Note `Picked up: Dnnn` on the item. A technical answer you decide and record as By Agent. A passing remark in a note that does not settle the question is input, not a decision.

## Subagents hand items to the coordinator

A subagent never touches the desk and never asks the developer in chat. Its handoff file ends with one line per item:

`FOR REVIEW: <review|question> | <area> | <title> | <what to judge, or the question with options and a recommendation> | files: <desk-ready paths>`

Desk-ready files: stills as PNG or JPEG under 15 MB; films already through the ffmpeg line above, under 15 MB, no audio; each with a caption (which shot, what changed), kept in an ignored folder or `~/.cache/terra-review`, never `/tmp`. The coordinator uploads them, adds the item, and puts the answer in the next agent's brief.
