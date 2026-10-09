#!/usr/bin/env python3
"""Merge the real-machine lanes' pull requests that keep to the rules.

Run by .github/workflows/merge-lanes.yml. It merges a pull request from a lane
branch (test/win-*, test/arm-*, and in filer test/linux-*) with a merge commit
pinned to its head, and only when all of these hold:

  1. it touches only the files a lane may write: the checklists and one new
     report under qa-reports/ (filer), or the checklists and new files under
     qa-reports/ (tsumugi);
  2. every changed checklist line is the same line with only its mark changed,
     from `[ ]` to a mark that lane may set;
  3. every new mark has its evidence: a line naming the row (or the key) in
     the pull request body -- in tsumugi the new report counts too -- and a
     `[~]` names its picture;
  4. filer: TODO.md's "マージで止めている実機の PR" does not hold it;
  5. every check run on its head has finished, none failed, and the
     checklist job is among them;
  6. GitHub says it merges without a conflict.

Anything else is left alone. A rule that fails, a red check or a conflict
gets one comment per head (a hidden marker keeps it from repeating), which
the merge routine reads (.claude/merge-routine.md). The merger's share --
version, CHANGELOG, the lane's queue -- stays with the routine.

It reads the pull request through the API only. It never checks out or runs
the pull request's code.

    python3 scripts/merge-lanes.py              # GITHUB_TOKEN, GITHUB_REPOSITORY
    MERGE_LANES_DRY_RUN=1 python3 scripts/merge-lanes.py
    python3 scripts/merge-lanes.py --self-test

The same file lives in uchmk/filer and uchmk/tsumugi; keep the two copies
identical (the repository picks its rules from RULES).
"""

import base64
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request

API = os.environ.get("GITHUB_API_URL", "https://api.github.com")
REPO = os.environ.get("GITHUB_REPOSITORY", "")
TOKEN = os.environ.get("GITHUB_TOKEN", "")
DRY = os.environ.get("MERGE_LANES_DRY_RUN", "") not in ("", "0", "false")

WINDOWS_MARKS = {
    "TESTING-CHECKS.md": {(" ", "x"), (" ", "~")},
    "TESTING-KEYS.md": {(" ", "x")},
}

RULES = {
    "uchmk/filer": {
        "lanes": {
            "test/win-": WINDOWS_MARKS,
            "test/arm-": WINDOWS_MARKS,
            "test/linux-": {"TESTING-LINUX.md": {(" ", "x"), (" ", "-")}},
        },
        # exactly one new report: another run's file is not this run's to edit
        "reports": (1, 1),
        "evidence_in_report": False,
        "required_checks": ["checklists"],
        "hold_heading": "マージで止めている実機の PR",
    },
    "uchmk/tsumugi": {
        "lanes": {"test/win-": WINDOWS_MARKS, "test/arm-": WINDOWS_MARKS},
        "reports": (0, 99),
        "evidence_in_report": True,
        "required_checks": ["check"],
        "hold_heading": None,
    },
}

OK_CONCLUSIONS = {"success", "skipped", "neutral"}
MARK_LINE = re.compile(r"^- \[(.)\] (.*)$")
PICTURE = re.compile(r"\.(png|jpe?g|bmp|gif|webp)\b", re.IGNORECASE)
MARKER = "<!-- merge-lanes:{kind}:{sha} -->"


class RuleError(Exception):
    pass


# ---------------------------------------------------------------- pure parts


def mark_pairs(patch):
    """The (old, new) line pairs of a unified diff that only replaces lines.

    Raises RuleError when a group of removed lines is not matched by as many
    added lines (a line added or removed, not changed)."""
    pairs, removed, added = [], [], []

    def flush():
        if len(removed) != len(added):
            raise RuleError(
                f"{len(removed)} line(s) removed and {len(added)} added in one place"
                " -- only changed marks may differ")
        pairs.extend(zip(removed, added))
        removed.clear()
        added.clear()

    for line in patch.split("\n"):
        if line.startswith("\\"):  # "\ No newline at end of file"
            continue
        if line.startswith("-"):
            if added:
                flush()
            removed.append(line[1:])
        elif line.startswith("+"):
            added.append(line[1:])
        else:  # a hunk header or a context line
            flush()
    flush()
    return pairs


def new_marks(path, patch, allowed):
    """[(mark, rest of the line)] for each line the patch re-marks.

    Raises RuleError for any change that is not one allowed mark change."""
    out = []
    for old, new in mark_pairs(patch):
        mo, mn = MARK_LINE.match(old), MARK_LINE.match(new)
        if not mo or not mn:
            raise RuleError(f"{path}: a changed line is not a checklist row: {new[:80]!r}")
        if mo.group(2) != mn.group(2):
            raise RuleError(f"{path}: a row's text changed, not only its mark: {new[:80]!r}")
        if (mo.group(1), mn.group(1)) not in allowed:
            raise RuleError(
                f"{path}: [{mo.group(1)}] -> [{mn.group(1)}] is not a lane's to make: {new[:80]!r}")
        out.append((mn.group(1), mn.group(2)))
    return out


def row_id(path, rest):
    """The row's id (`**25.4e**`), or for TESTING-KEYS.md the key (`<Esc>`)."""
    if path == "TESTING-KEYS.md":
        m = re.match(r"`([^`]+)`", rest)
    else:
        m = re.match(r"\*\*([^*]+)\*\*", rest)
    if not m:
        raise RuleError(f"{path}: no id at the start of {rest[:80]!r}")
    return m.group(1)


RANGE = re.compile(r"(\d+)\.(\d+)\s*(?:-|–|—|〜|~|to)\s*(?:(\d+)\.)?(\d+)(?![\w.])")


def mentions(line, rid, key):
    """Whether an evidence line names this row or key."""
    if key:
        return f"`{rid}`" in line
    if f"**{rid}**" in line:
        return True
    if re.search(r"(?<![\w.])" + re.escape(rid) + r"(?![\w]|\.\d)", line):
        return True
    m = re.fullmatch(r"(\d+)\.(\d+)", rid)
    if m:  # "2.32-2.35", "2.4〜2.7", "49.9-49.11"
        major, n = m.group(1), int(m.group(2))
        for r in RANGE.finditer(line):
            if r.group(1) == major and r.group(3) in (None, major) \
                    and int(r.group(2)) <= n <= int(r.group(4)):
                return True
    return False


def missing_evidence(ticks, text):
    """The ticks with no evidence line in `text`, each with what is missing.

    `ticks` is [(path, mark, id)]. An evidence line names the row and says
    something beyond it; a `[~]` also names its picture somewhere on a line
    that names the row."""
    lines = text.split("\n")
    out = []
    for path, mark, rid in ticks:
        key = path == "TESTING-KEYS.md"
        found = [ln for ln in lines if mentions(ln, rid, key) and len(ln.strip()) >= len(rid) + 10]
        if not found:
            out.append(f"{rid} [{mark}] ({path}): no evidence line")
        elif mark == "~" and not any(PICTURE.search(ln) for ln in found):
            out.append(f"{rid} [~] ({path}): no picture named on its evidence line")
    return out


def held_numbers(todo, heading):
    """The pull request numbers named under TODO.md's hold heading."""
    out, inside = set(), False
    for line in todo.split("\n"):
        if line.startswith("#"):
            inside = line.lstrip("#").strip() == heading
            continue
        if inside:
            out.update(int(n) for n in re.findall(r"#(\d+)(?!\d)", line))
    return out


def check_verdict(runs, statuses, required, own):
    """'green', 'pending' or 'red: <names>' for a head's check runs.

    Re-runs leave older runs of the same name behind: the newest counts."""
    latest = {}
    for r in runs:
        if r["name"] in own:
            continue
        k = (r["name"], (r.get("app") or {}).get("id"))
        if k not in latest or r["id"] > latest[k]["id"]:
            latest[k] = r
    runs = list(latest.values())
    red = sorted(r["name"] for r in runs
                 if r["status"] == "completed" and r["conclusion"] not in OK_CONCLUSIONS)
    red += [s["context"] for s in statuses if s["state"] in ("failure", "error")]
    if red:
        return "red: " + ", ".join(red)
    if any(r["status"] != "completed" for r in runs) \
            or any(s["state"] == "pending" for s in statuses):
        return "pending"
    names = {r["name"] for r in runs if r["conclusion"] == "success"}
    if any(n not in names for n in required):
        return "pending"  # not started yet
    return "green"


# ---------------------------------------------------------------- GitHub


def api(method, path, body=None):
    url = path if path.startswith("http") else API + path
    req = urllib.request.Request(url, method=method)
    req.add_header("Authorization", f"Bearer {TOKEN}")
    req.add_header("Accept", "application/vnd.github+json")
    req.add_header("X-GitHub-Api-Version", "2022-11-28")
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, data, timeout=60) as resp:
            raw = resp.read()
            link = resp.headers.get("Link", "")
            return resp.status, (json.loads(raw) if raw else None), link
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            payload = json.loads(raw)
        except ValueError:
            payload = {"message": raw.decode(errors="replace")}
        return e.code, payload, ""


def get(path):
    status, payload, _ = api("GET", path)
    if status != 200:
        raise RuntimeError(f"GET {path}: {status} {payload}")
    return payload


def get_all(path, key=None):
    out, url = [], path + ("&" if "?" in path else "?") + "per_page=100"
    while url:
        status, payload, link = api("GET", url)
        if status != 200:
            raise RuntimeError(f"GET {url}: {status} {payload}")
        out.extend(payload[key] if key else payload)
        m = re.search(r'<([^>]+)>;\s*rel="next"', link)
        url = m.group(1) if m else None
    return out


def raw_file(path, ref):
    status, payload, _ = api("GET", f"/repos/{REPO}/contents/{path}?ref={ref}")
    if status != 200:
        return ""
    return base64.b64decode(payload.get("content", "")).decode("utf-8", errors="replace")


def comment_once(pr, kind, sha, text):
    marker = MARKER.format(kind=kind, sha=sha)
    for c in get_all(f"/repos/{REPO}/issues/{pr['number']}/comments"):
        if marker in (c.get("body") or ""):
            return
    body = f"{marker}\n**merge-lanes** did not merge this ({kind}, head `{sha[:7]}`):\n\n{text}\n\n" \
           "The merge routine reads this and takes it from here (`.claude/merge-routine.md`)."
    if DRY:
        print(f"  (dry run) would comment: {kind}: {text}")
        return
    api("POST", f"/repos/{REPO}/issues/{pr['number']}/comments", {"body": body})


# ---------------------------------------------------------------- one pull request


def rule_problems(pr, rules, allowed_marks, head):
    """What breaks the file, mark and evidence rules (empty: none)."""
    files = get_all(f"/repos/{REPO}/pulls/{pr['number']}/files")
    problems, ticks, reports = [], [], []
    for f in files:
        name, status = f["filename"], f["status"]
        if name.startswith("qa-reports/"):
            if status != "added":
                problems.append(f"{name}: {status} -- a run only adds its own report")
            else:
                reports.append(name)
            continue
        if name not in allowed_marks:
            problems.append(f"{name}: not a file this lane may change")
            continue
        if status != "modified" or "patch" not in f:
            problems.append(f"{name}: {status}, no line diff to read")
            continue
        try:
            for mark, rest in new_marks(name, f["patch"], allowed_marks[name]):
                ticks.append((name, mark, row_id(name, rest)))
        except RuleError as e:
            problems.append(str(e))
    lo, hi = rules["reports"]
    if not lo <= len(reports) <= hi:
        problems.append(f"{len(reports)} new file(s) under qa-reports/; expected {lo}" +
                        (f" to {hi}" if hi != lo else ""))
    if not problems:
        text = pr.get("body") or ""
        if rules["evidence_in_report"]:
            text += "\n" + "\n".join(raw_file(r, head) for r in reports if r.endswith(".md"))
        problems += missing_evidence(ticks, text)
    return problems, ticks


def mergeable(number):
    """GitHub's `mergeable` for a pull request, waiting a little while it is computed."""
    for _ in range(6):
        pr = get(f"/repos/{REPO}/pulls/{number}")
        if pr["mergeable"] is not None:
            return pr
        time.sleep(5)
    return pr


def handle(pr, rules, allowed_marks, held, own_checks):
    n, head = pr["number"], pr["head"]["sha"]
    if n in held:
        return "held in TODO.md"
    problems, ticks = rule_problems(pr, rules, allowed_marks, head)
    if problems:
        comment_once(pr, "rules", head, "\n".join(f"- {p}" for p in problems))
        return "rules: " + "; ".join(problems)
    runs = get_all(f"/repos/{REPO}/commits/{head}/check-runs", "check_runs")
    statuses = get(f"/repos/{REPO}/commits/{head}/status")["statuses"]
    verdict = check_verdict(runs, statuses, rules["required_checks"], own_checks)
    if verdict.startswith("red"):
        comment_once(pr, "red", head, f"- {verdict}")
        return verdict
    if verdict != "green":
        return "checks still running"
    fresh = mergeable(n)
    if fresh["head"]["sha"] != head:
        return "head moved while checking; next time"
    if fresh["mergeable"] is None:
        return "GitHub has not worked out mergeability yet; next time"
    if not fresh["mergeable"]:
        comment_once(pr, "conflict", head, "- it conflicts with the base branch")
        return "conflict"
    if DRY:
        return f"(dry run) would merge {head[:7]} ({len(ticks)} marks)"
    status, payload, _ = api("PUT", f"/repos/{REPO}/pulls/{n}/merge",
                             {"merge_method": "merge", "sha": head})
    if status != 200 or not payload.get("merged"):
        return f"merge refused: {status} {payload.get('message')}"
    return f"merged {head[:7]} ({len(ticks)} marks)"


def main():
    rules = RULES.get(REPO)
    if not rules:
        sys.exit(f"merge-lanes: no rules for {REPO!r}")
    if not TOKEN:
        sys.exit("merge-lanes: GITHUB_TOKEN is not set")
    own_checks = {os.environ.get("MERGE_LANES_JOB", "merge")}
    held = set()
    if rules["hold_heading"]:
        default = get(f"/repos/{REPO}")["default_branch"]
        held = held_numbers(raw_file("TODO.md", default), rules["hold_heading"])
    prs = get_all(f"/repos/{REPO}/pulls?state=open&sort=created&direction=asc")
    for pr in prs:
        ref = pr["head"]["ref"]
        lane = next((p for p in rules["lanes"] if ref.startswith(p)), None)
        if lane is None:
            continue
        if pr["draft"] or (pr["head"].get("repo") or {}).get("full_name") != REPO:
            print(f"#{pr['number']} {ref}: skipped (draft or from a fork)")
            continue
        try:
            outcome = handle(pr, rules, rules["lanes"][lane], held, own_checks)
        except RuntimeError as e:
            outcome = f"error: {e}"
        print(f"#{pr['number']} {ref}: {outcome}")


# ---------------------------------------------------------------- self-test


def self_test():
    patch = ("@@ -38,4 +38,4 @@\n - [x] **1.1** a\n-- [ ] **1.2** b\n-- [ ] **1.3** c\n"
             "+- [x] **1.2** b\n+- [~] **1.3** c\n - [ ] **1.4** d\n"
             "@@ -90,2 +90,2 @@\n-- [ ] **49.10** e\n+- [x] **49.10** e\n\\ No newline at end of file")
    marks = new_marks("TESTING-CHECKS.md", patch, WINDOWS_MARKS["TESTING-CHECKS.md"])
    assert marks == [("x", "**1.2** b"), ("~", "**1.3** c"), ("x", "**49.10** e")], marks
    for bad, why in [
        ("@@\n-- [ ] **1.2** b\n+- [x] **1.2** B\n", "text changed"),
        ("@@\n - [ ] **1.2** b\n+- [x] **1.3** c\n", "line added"),
        ("@@\n-- [x] **1.2** b\n+- [ ] **1.2** b\n", "untick"),
        ("@@\n-- [~] **1.2** b\n+- [x] **1.2** b\n", "[~] to [x] is the owner's"),
        ("@@\n-## 1. Pane\n+## 1. Panes\n", "not a row"),
    ]:
        try:
            new_marks("TESTING-CHECKS.md", bad, WINDOWS_MARKS["TESTING-CHECKS.md"])
        except RuleError:
            continue
        raise AssertionError(f"accepted: {why}")
    try:
        new_marks("TESTING-KEYS.md", "@@\n-- [ ] `q` — Quit\n+- [~] `q` — Quit\n",
                  WINDOWS_MARKS["TESTING-KEYS.md"])
        raise AssertionError("accepted [~] in TESTING-KEYS.md")
    except RuleError:
        pass

    assert row_id("TESTING-CHECKS.md", "**25.4e** text") == "25.4e"
    assert row_id("TESTING-KEYS.md", "`<C-q>` — Quit the process · `quit`") == "<C-q>"
    assert row_id("TESTING-KEYS.md", "`Ctrl+Shift+T` · `Cmd+T` — New session") == "Ctrl+Shift+T"

    assert mentions("- **25.4e** `[x]`: shows v7.5.4.0", "25.4e", False)
    assert mentions("- 2.28: Ctrl+Shift+Up x3 put CMD5 at the top", "2.28", False)
    assert not mentions("- 2.28: ...", "2.2", False)
    assert not mentions("- **1.1a** ...", "1.1", False)
    assert not mentions("- 12.28: ...", "2.28", False)
    assert mentions("- 2.32-2.35: all four held as written", "2.33", False)
    assert mentions("- 49.9-49.11 held on the new pane", "49.10", False)
    assert mentions("- 2.4〜2.7 looked right in the picture", "2.4", False)
    assert not mentions("- 2.32-2.35: ...", "2.36", False)
    assert not mentions("- 3.32-3.35: ...", "2.33", False)
    assert mentions("- `Ctrl+Shift+T`: key log `Some(NewTab)`", "Ctrl+Shift+T", True)
    assert not mentions("- `Ctrl+Shift+Tab`: ...", "Ctrl+Shift+T", True)

    body = ("## Evidence\n- **1.2** `[x]`: printed `ok` twice, exit 0\n"
            "- **1.3** `[~]`: the block cursor, `C:\\ev\\run\\1.3-cursor.png`, no hollow box\n"
            "- **1.4** `[~]`: looked fine\n- **1.5**\n")
    ticks = [("TESTING-CHECKS.md", "x", "1.2"), ("TESTING-CHECKS.md", "~", "1.3"),
             ("TESTING-CHECKS.md", "~", "1.4"), ("TESTING-CHECKS.md", "x", "1.5"),
             ("TESTING-CHECKS.md", "x", "1.6")]
    miss = missing_evidence(ticks, body)
    assert [m.split(" ")[0] for m in miss] == ["1.4", "1.5", "1.6"], miss

    todo = ("## A\n- #12 not this\n## マージで止めている実機の PR\n\n説明（#3 の例）\n\n"
            "- #311: 32.20 を…\n  - 持ち主の答え: #312 と同じ\n## B\n- #400\n")
    assert held_numbers(todo, "マージで止めている実機の PR") == {3, 311, 312}
    assert held_numbers("## マージで止めている実機の PR\n\nいまは無い\n", "マージで止めている実機の PR") == set()

    def run(name, status, conclusion, rid=1, app=1):
        return {"name": name, "status": status, "conclusion": conclusion, "id": rid, "app": {"id": app}}
    green = [run("checklists", "completed", "success"), run("audit", "completed", "skipped", 2)]
    assert check_verdict(green, [], ["checklists"], {"merge"}) == "green"
    assert check_verdict(green, [], ["clippy"], {"merge"}) == "pending"
    assert check_verdict(green + [run("test", "in_progress", None, 3)], [], [], set()) == "pending"
    assert check_verdict(green + [run("test", "completed", "failure", 3)], [], [], set()) == "red: test"
    rerun = [run("test", "completed", "failure", 3), run("test", "completed", "success", 4)]
    assert check_verdict(green + rerun, [], [], set()) == "green"
    assert check_verdict(green + [run("merge", "in_progress", None, 5)], [], [], {"merge"}) == "green"
    assert check_verdict(green, [{"context": "ci/x", "state": "failure"}], [], set()) == "red: ci/x"
    print("merge-lanes self-test: OK")


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        self_test()
    else:
        main()
