# tools/git

Checks of git history that the hooks run.

- `undone.ts`, run by `.githooks/pre-push` (D431): `node tools/git/undone.ts [range]` (default `origin/main..HEAD`) warns, a line an undo, of a commit that silently undoes another commit of the last 48 hours outside the range: it deletes every line that commit added to a file (at least three) and puts back what it replaced, or the commit only added, while its message names neither the file nor the commit. Moves and `spike/` don't count. It never fails; nothing printed means clean. Node 22.18+, no dependencies; `node --test tools/git/undone.test.ts`.
