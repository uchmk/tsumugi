# The merge routine

A claude.ai Routine starts a new cloud session at :40 every hour (JST) and
tells it to read this file and do what it says. It merges the pull requests
of the real-machine lanes (`test/win-*` from the x64 machine, `test/arm-*`
from the ARM64 laptop; [windows-role.md](windows-role.md) says how they are
made) and does the merger's share on `main`, which the lanes are told to
leave to whoever merges.

Nobody is watching and nothing is remembered from the last run: never wait
for input. Reply in Japanese; commits, the CHANGELOG's code names and the
role file in English (CHANGELOG.md itself is Japanese). Read
[CLAUDE.md](../CLAUDE.md) first; its rules hold here in full.

## 0. Before anything

- No open pull request from `test/win-*` or `test/arm-*`: reply
  `nothing to do` and stop. Do not read further, build or check out.
- The checkout: `uchmk/tsumugi` with push access (attach it with `add_repo`,
  `access: "push"`, and clone it when the session does not have it), then
  `git fetch origin main && git checkout -B claude/merge-run origin/main`.
- GitHub from here: the GitHub tools the session has, or `gh`.

## 1. Each open pull request, oldest first

Merge it only when **all** of these hold. When one does not, the line in the
reply says which.

1. **Its checks are done and none failed**: every check run on the head
   commit is `completed`, none `failure` / `cancelled` / `timed_out`. One
   still running: leave the pull request for the next hour (`waiting on CI`);
   do not wait in this session. A Markdown-only pull request runs only the
   Checklists workflow; that is enough.
2. **It touches only** `TESTING-CHECKS.md`, `TESTING-KEYS.md` and files under
   `qa-reports/`. Anything else (a script, TESTING.md, the role, code): do
   not merge; say so in the reply and in one comment on the pull request.
3. **The checklists only change marks**: each changed line of the two
   checklists differs from `main`'s only in `[ ]` -> `[x]` or `[ ]` -> `[~]`.
   A reworded row, a row removed, a `[x]` taken back: do not merge.
4. **Every new `[x]` has its evidence**: the pull request body or its report
   names the row with the command and what came back (windows-role.md,
   "Ticking"). A tick with no line for it: do not merge; comment which rows.
   A `[~]` needs the picture's name instead.

Then merge with a merge commit, pinned to the head you checked:
`merge_method=merge` and `sha=<the full head SHA>` (the GitHub tool's merge,
or `gh api -X PUT repos/uchmk/tsumugi/pulls/N/merge -f merge_method=merge -f sha=…`).
A pull request merged in the meantime is passed over.

**It conflicts with `main`** (usually both lanes ticked rows next to each
other, or `main` reworded a row): on its branch, `git merge origin/main`
(a merge commit; never rebase or force-push), and for each conflicting line
keep a tick from either side, but take `main`'s line, unticked, where
`main` changed the row's words. Regenerate with
`cargo run -q -p tsumugi --example make-testcheck` and `make-keycheck`, check
both with `-- --check`, commit, push to the pull request's branch, and leave
the merge to the next hour (its checks run again).

## 2. The merger's share (once per run, after the merges)

On `claude/merge-run`, after `git fetch origin main && git reset --hard origin/main`:

1. **Version**: one PATCH bump for the run in `Cargo.toml`
   (`[workspace.package]`), then `cargo build -q --workspace` so
   `Cargo.lock` follows.
2. **CHANGELOG.md**: a new section at the top under `## [未リリース]`,
   dated `TZ=Asia/Tokyo date +%F`, `### 変更`, one Japanese line per merged
   pull request from the changelog line in its body (rows ticked on which
   machine, e.g. "実機（x64）で 1.3・1.5 を確かめた（#N）").
3. **Re-tests**: in windows-role.md's "Re-tests of changed behaviour" row,
   take out the ids the merged pull requests ticked; when none is left, the
   cell reads `none yet`.
4. **`## Queue` in a pull request body**: a row to offer again goes into the
   re-tests cell. Anything else it asks of the queue (another order, a
   section skipped) goes to TODO.md under "実機のレーンから", for an
   interactive session: do not edit the rest of windows-role.md.
5. **The reports' `### Proposals` and the bugs they found**: one line each in
   TODO.md under "実機のレーンから" (`- （実機 #N）…`, with the report's path).
   Do not fix them. A row that **failed** on ARM64 goes first.
6. `scripts/verify.sh`; the last line must be `ALL OK: …`.
7. Commit `vX.Y.Z: Merge real-machine checks for <rows>` with a one-paragraph
   English body (it becomes the release note), and `git push origin HEAD:main`.
   Rejected because `main` moved: `git fetch`, `git reset --hard origin/main`,
   redo 1 to 7. Never `--force`.

Do not wait for `main`'s CI after the push. When nothing was merged, there is
no share and no push.

## The reply

In Japanese, one line per pull request: `merged #N` /
`waiting on CI for #N` / `not merged #N: why` / `conflict merged into #N, next run`;
then the version pushed (`v0.74.6 を push`), or `nothing to do` when no
pull request was open.
