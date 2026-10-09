# The merge routine

A claude.ai Routine starts a new cloud session at :40 every hour (JST) and
tells it to read this file and do what it says. It does the merger's share on
`main` for the pull requests of the real-machine lanes (`test/win-*` from the
x64 machine, `test/arm-*` from the ARM64 laptop; [windows-role.md](windows-role.md)
says how they are made), which the lanes are told to leave to whoever merges.

**It does not merge** (v0.76.4). The `Merge lanes` workflow
(`.github/workflows/merge-lanes.yml`, `scripts/merge-lanes.py`, the same
file as filer's) merges each lane pull request with a merge commit pinned to
its head once it keeps to the rules in 1 and its checks are green. In filer
the cloud session's auto mode refused the routine's merges as unreviewed
(2026-10-10); a workflow whose rules are code does not need a reviewer for
each one. Do not merge, and do not look for another way to: a merge the
workflow will not make is the owner's.

A lane's next run (`scripts/auto-wintest.ps1`) waits while one of its pull
requests is open, and also while its latest merged one is not named in
`main`'s CHANGELOG.md. **So the share is what lets the lane go on.**

Nobody is watching and nothing is remembered from the last run: never wait
for input. Reply in Japanese; commits, the CHANGELOG's code names and the
role file in English (CHANGELOG.md itself is Japanese). Read
[CLAUDE.md](../CLAUDE.md) first; its rules hold here in full.

## 0. Before anything

- Find the work:
  - **merged without a share**: lane pull requests merged in the last 7 days
    whose `#N` is nowhere in `origin/main`'s CHANGELOG.md
    (`gh api "repos/uchmk/tsumugi/pulls?state=closed&sort=updated&direction=desc&per_page=50"`,
    those with `merged_at` set). Each gets its share in 2. Every merged lane
    pull request's `（#N）` must end up in CHANGELOG.md.
  - **open, and stopped**: open lane pull requests the workflow commented on.
    Its comments end in a hidden marker `<!-- merge-lanes:<kind>:<sha> -->`,
    one per head and kind: `rules`, `red` or `conflict`. One whose marker
    names an older head than the pull request's is stale.
  - Open lane pull requests with no such comment are the workflow's (checks
    running, or it merges them on its next pass: whenever CI, Checklists or
    Audit finishes, and at :17). Leave them.
- Nothing in either list: reply `nothing to do` and stop. Do not read
  further, build or check out.
- The checkout: `uchmk/tsumugi` with push access (attach it with `add_repo`,
  `access: "push"`, and clone it when the session does not have it), then
  `git fetch origin main && git checkout -B claude/merge-run origin/main`.
- GitHub from here: the GitHub tools the session has, or `gh api` (GraphQL
  may be unavailable).

## 1. What the workflow merges, and what to do when it stops

All of these, or the workflow does not merge (so that you can tell the owner
why):

1. **It touches only** `TESTING-CHECKS.md`, `TESTING-KEYS.md` and new files
   under `qa-reports/` (added, not edited).
2. **The checklists only change marks**: each changed line differs from
   `main`'s only in `[ ]` -> `[x]` or `[ ]` -> `[~]`, in both checklists
   (keys may be `[~]` since v0.76.5). A reworded row, a row removed, a `[x]` taken back: no.
3. **Every new mark has its evidence**: a line in the pull request body or
   its report that names the row (`**2.28**`, `2.28`, a range like
   `2.28-2.30`, or for TESTING-KEYS.md the key in backticks) and says more
   than the name. A `[~]` also needs the picture's file name on that line.
4. **The `check` job (Checklists) is green** on the head, no other check
   failed, and it does not conflict with `main`.

When it stops:

- **`conflict`** (usually both lanes ticked rows next to each other, or
  `main` reworded a row): on its branch, `git merge origin/main` (a merge
  commit; never rebase or force-push), and for each conflicting line keep a
  tick from either side, but take `main`'s line, unticked, where `main`
  changed the row's words. Regenerate with
  `cargo run -q -p tsumugi --example make-testcheck` and `make-keycheck`,
  check both with `-- --check`, commit, and push to the pull request's
  branch. The workflow merges it once its checks are green again; its share
  falls to the next run.
- **`rules`**: you do not change a run's ticks or its report. Add a question
  to QUESTIONS.md (CLAUDE.md's format, one recommended option): the pull
  request, each problem the comment lists, and the choices (the owner merges
  it by hand as it is, or closes it and the rows are offered again). Commit
  it with the share in 2; a run with only a question still bumps PATCH and
  adds a CHANGELOG line (one version per push).
- **`red`**: read the log. A pull request of Markdown cannot break a build.
  **Do not fix code from here**: write the failing check, the log line and
  your reading of the cause into TODO.md under "実機のレーンから", as part of
  the share.
- A question already open for that pull request: do not add another. One the
  owner answered: carry it out (close the pull request with a one-line
  comment naming the answer, or leave a merge to the owner) and mark the
  question answered.

## 2. The merger's share (once per run)

On `claude/merge-run`, after `git fetch origin main && git reset --hard origin/main`:

1. **Version**: one PATCH bump for the run in `Cargo.toml`
   (`[workspace.package]`), then `cargo build -q --workspace` so
   `Cargo.lock` follows.
2. **CHANGELOG.md**: a new section at the top under `## [未リリース]`,
   dated `TZ=Asia/Tokyo date +%F`, `### 変更`, one Japanese line per merged
   pull request found in 0 without a share, from the changelog line in its body (rows ticked on which
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

Do not wait for `main`'s CI after the push. When there is nothing to share and
nothing to write, there is no push. The share's push is also what runs CI on
`main` after the workflow's merges (a merge made with the workflow's token
starts no workflows).

## The reply

In Japanese, one line per pull request: `shared #N` /
`resolved conflict on #N` / `asked about #N: why（QUESTIONS.md Qn）` /
`closed #N: …`; then the version pushed (`v0.76.5 を push`), or
`nothing to do` when there was nothing.
