# The merge routine

A claude.ai Routine starts a new cloud session and tells it to read this file
and do what it says. It looks after the pull requests of the real-machine
lanes (`test/win-*` from the x64 machine, `test/arm-*` from the ARM64 laptop;
[windows-role.md](windows-role.md) says how they are made) **where the
workflow stopped**.

**It does not merge, and it does not do the share** (v0.76.6). The `Merge
lanes` workflow (`.github/workflows/merge-lanes.yml`, `scripts/merge-lanes.py`,
the same file as filer's) does both:

- it merges each lane pull request with a merge commit pinned to its head once
  it keeps to the rules in 1 and its checks are green;
- a pull request that keeps to them but conflicts only in the checklists, it
  merges itself: `main`'s file with the pull request's marks made again on the
  rows `main` still has unticked and unchanged, checked to be marks only, and
  pushed to `main`. A mark whose row `main` reworded is dropped and named in
  a comment (`<!-- merge-lanes:dropped:<sha> -->`);
- then the share for every merged lane pull request whose `#N` is not in
  CHANGELOG.md: one PATCH bump (Cargo.toml and Cargo.lock), a CHANGELOG line
  per pull request made from its marks, the ticked rows out of the role's
  re-test list, the reports' `### Proposals` and the `## Queue` notes into
  TODO.md under "実機のレーンから" (`- [ ] （実機 #N）…`). It pushes that to
  `main` and starts CI and Checklists there.

In filer the cloud session's auto mode refused the routine's merges as
unreviewed (2026-10-10); a workflow whose rules are code does not need a
reviewer for each one. Do not merge, and do not look for another way to: a
merge the workflow will not make is the owner's.

A lane's next run (`scripts/auto-wintest.ps1`) waits while one of its pull
requests is open, and also while its latest merged one is not named in
`main`'s CHANGELOG.md; the workflow's share usually lands minutes after the
merge.

Nobody is watching and nothing is remembered from the last run: never wait
for input. Reply in Japanese; commits, the CHANGELOG's code names and the
role file in English (CHANGELOG.md itself is Japanese). Read
[CLAUDE.md](../CLAUDE.md) first; its rules hold here in full.

## 0. Before anything

- Find the work:
  - **open, and stopped**: open lane pull requests the workflow commented on.
    Its comments end in a hidden marker `<!-- merge-lanes:<kind>:<sha> -->`,
    one per head and kind: `rules`, `red` or `conflict`. One whose marker
    names an older head than the pull request's is stale.
  - **merged without a share** for more than an hour: lane pull requests
    merged in the last 7 days whose `#N` is nowhere in `origin/main`'s
    CHANGELOG.md
    (`gh api "repos/uchmk/tsumugi/pulls?state=closed&sort=updated&direction=desc&per_page=50"`,
    those with `merged_at` set). The workflow should have shared them: read
    its last runs' logs (Actions, `Merge lanes`), and do that share by hand as
    in 2.
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
   failed, and it does not conflict with `main` (a conflict only in the
   checklists it resolves itself, without waiting for the checks, which do
   not start on a conflicting pull request).

When it stops:

- **`conflict`**: the workflow resolves conflicts in the checklists itself,
  so this one is in another file (usually two reports with one name). On its
  branch, `git merge origin/main` (a merge
  commit; never rebase or force-push), and for each conflicting line keep a
  tick from either side, but take `main`'s line, unticked, where `main`
  changed the row's words. Regenerate with
  `cargo run -q -p tsumugi --example make-testcheck` and `make-keycheck`,
  check both with `-- --check`, commit, and push to the pull request's
  branch. The workflow merges and shares it once its checks are green again.
- **`rules`**: you do not change a run's ticks or its report. Add a question
  to QUESTIONS.md (CLAUDE.md's format, one recommended option): the pull
  request, each problem the comment lists, and the choices (the owner merges
  it by hand as it is, or closes it and the rows are offered again). Commit
  it as in 2; a run with only a question still bumps PATCH and adds a
  CHANGELOG line (one version per push).
- **`red`**: read the log. A pull request of Markdown cannot break a build.
  **Do not fix code from here**: write the failing check, the log line and
  your reading of the cause into TODO.md under "実機のレーンから", committed as
  in 2.
- A question already open for that pull request: do not add another. One the
  owner answered: carry it out (close the pull request with a one-line
  comment naming the answer, or leave a merge to the owner) and mark the
  question answered.

## 2. Committing (once per run)

What you write (questions, TODO lines, and a share the workflow missed) goes
in one commit on `claude/merge-run`, after
`git fetch origin main && git reset --hard origin/main`:

1. **Version**: one PATCH bump in `Cargo.toml` (`[workspace.package]`), then
   `cargo build -q --workspace` so `Cargo.lock` follows.
2. **CHANGELOG.md**: a new section at the top under `## [未リリース]`, dated
   `TZ=Asia/Tokyo date +%F`, `### 変更`, one Japanese line for what you did.
   A share by hand also needs the workflow's lines: one per merged pull
   request (rows ticked on which machine, e.g.
   "実機（x64）で 1.3・1.5 を確かめた（#N）"), the ticked rows out of
   windows-role.md's "Re-tests of changed behaviour" row (`none yet` when
   it empties), and the reports' `### Proposals` and `## Queue` lines in
   TODO.md under "実機のレーンから". Do not edit the rest of windows-role.md.
3. `scripts/verify.sh`; the last line must be `ALL OK: …`.
4. Commit `vX.Y.Z: <what>` with a one-paragraph English body (it becomes the
   release note), and `git push origin HEAD:main`. Rejected because `main`
   moved: `git fetch`, `git reset --hard origin/main`, redo 1 to 4. Never
   `--force`.

Do not wait for `main`'s CI after the push. When there is nothing to write,
there is no push.

## The reply

In Japanese, one line per pull request: `shared #N by hand` /
`resolved conflict on #N` / `asked about #N: why（QUESTIONS.md Qn）` /
`closed #N: …`; then the version pushed (`v0.76.5 を push`), or
`nothing to do` when there was nothing.
