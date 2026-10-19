# Review desk

A claude.ai artifact page where one developer answers the questions and reviews the stills and films their agents raise, from any machine. Each developer publishes their own copy and their own coordinator keeps it; there is no shared desk and no server of ours (D422). The playbook is the `review-desk` skill (`.agents/skills/review-desk/SKILL.md`).

- `index.html`: the page, one file, no personal data. Open items first, oldest first; an item's options, stills and videos show inline, and answering writes the answer back to the item.
- Published with the Artifact tool and the capabilities `{db: {}, user: {}, assets: {}}`; the page reads and writes the `items` collection of its own database.
- Items are added by the coordinator with the ArtifactData tool; stills and videos are uploaded as the artifact's assets (each under 15 MB) and referenced by the URL the upload returns.
- The page's URL is kept in `~/.config/terra/desk.json`, never committed.

An item is one document in `items`:

```json
{"kind": "question|review", "area": "…", "status": "open|answered", "createdAt": "ISO time",
 "title": "…", "body": "…", "options": [{"label": "…", "detail": "…"}],
 "ask": "one sentence: what to decide or look at",
 "images": [{"url": "…", "caption": "…"}], "videos": [{"url": "…", "caption": "…"}],
 "answer": "", "note": "", "answeredAt": ""}
```

The page sets `status`, `answer` (the option picked), `note`, `answeredAt` and `answeredBy` when the developer sends an answer. Answers do not notify the session: the coordinator reads them each turn.
