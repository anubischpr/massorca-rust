import io, re, sys, glob, os, datetime
from collections import OrderedDict, defaultdict

# Usage: python coverage_assemble.py <repo-root> <rows-dir> <commit> [<previous-commit>]
# <rows-dir> holds .md files of reviewer rows: | <ID> | <status> | <evidence> | <note> |
# With <previous-commit>, the current docs/inventory/coverage.md is read as the earlier
# measurement before it is overwritten, and a "Changed since" section compares the two.
WT, S_ROWS, COMMIT = sys.argv[1], sys.argv[2], sys.argv[3]
PREV = sys.argv[4] if len(sys.argv) > 4 else None
# One paragraph under the headline, written for each measurement.
READING = io.open(os.path.join(S_ROWS, "reading.txt"), encoding="utf-8").read().strip() if os.path.exists(os.path.join(S_ROWS, "reading.txt")) else ""
STAT = ["done", "partial", "in review", "not started", "changed", "dropped"]

# inventory rows
inv = OrderedDict(); domains = OrderedDict()
for l in io.open(os.path.join(WT, "docs/inventory/README.md"), encoding="utf-8"):
    l = l.rstrip("\n")
    m = re.match(r"^\| \[(.*)\]\((.*)\) \| ([A-Z]+) \| \d+", l)
    if m: domains[m.group(3)] = (m.group(1), m.group(2))
    if re.match(r"^\| [A-Z]+-\d+ \|", l):
        c = l[2:-2].split(" | ")
        inv[c[0]] = dict(name=c[1], mark=c[6], size=c[4])
fv = set()
for l in io.open(os.path.join(WT, "docs/inventory/first-version.md"), encoding="utf-8"):
    m = re.match(r"^- \*\*([A-Z]+-\d+)\*\*", l)
    if m: fv.add(m.group(1))

# coverage rows
cov = {}
for f in sorted(glob.glob(os.path.join(S_ROWS, "*.md"))):
    for l in io.open(f, encoding="utf-8"):
        l = l.strip()
        if not re.match(r"^\| [A-Z]+-\d+ \|", l): continue
        c = [x.strip() for x in l[2:-2].split(" | ")]
        assert len(c) == 4, (f, l[:80])
        assert c[1] in STAT, (c[0], c[1])
        assert c[0] not in cov, ("dup", c[0])
        cov[c[0]] = c
missing = [i for i in inv if i not in cov]; extra = [i for i in cov if i not in inv]
assert not missing and not extra, (missing[:20], extra[:20])
assert len(fv) == sum(1 for i in inv if inv[i]["mark"] == "first version"), (len(fv),)

# milestones (method documented in the file)
M1 = {"WS", "TERM", "GIT"}; M2 = {"SRV", "REM", "CLD"}; M3 = {"AG", "STAT", "ACC", "HIST", "AUTO"}
OV = {"SET-01": "M1", "SET-02": "M1", "SET-03": "M1", "SET-04": "M1", "SET-08": "M3", "CUS-01": "M1", "CUS-04": "M3", "CUS-08": "M1",
      "CLI-01": "M1", "CLI-03": "M1", "CLI-08": "M1", "CLI-09": "M1", "CLI-10": "M1", "CLI-11": "M1", "CLI-35": "M1",
      "CLI-04": "M2", "CLI-05": "M2", "CLI-12": "M3", "CLI-14": "M3", "CLI-23": "M3", "CLI-24": "M3", "CLI-25": "M3", "CLI-32": "M3",
      "PLAT-07": "M1", "PLAT-14": "M1", "PLAT-18": "M1", "PLAT-27": "M1"}
def milestone(i):
    if inv[i]["mark"] != "first version": return "M6"
    if i in OV: return OV[i]
    p = i.split("-")[0]
    return "M1" if p in M1 else "M2" if p in M2 else "M3" if p in M3 else "M4"

def tally(ids, status=None):
    # status: optional {ID: status} map (an earlier measurement); its notes are not known, so no "cb"
    t = defaultdict(int)
    for i in ids:
        s = status[i] if status else cov[i][1]
        t[s] += 1
        n = "" if status else cov[i][3].lower()
        if s == "changed" and "built" in n and "not built" not in n and "in review" not in n: t["cb"] += 1
    t["n"] = len(ids)
    scope = t["n"] - t["dropped"] - t["changed"]
    t["scope"] = scope
    pct = lambda x: f"{100 * x / scope:.1f}%" if scope else "-"
    t["p_cb"] = (f"{100 * (t['done'] + t['cb']) / (scope + t['cb']):.1f}%" if scope + t["cb"] else "-")
    t["p_done"] = pct(t["done"]); t["p_dr"] = pct(t["done"] + t["in review"]); t["p_w"] = pct(t["done"] + 0.5 * t["partial"])
    return t
HDR = "| Scope | Features | Done | Partial | In review | Not started | Changed | Dropped | Done % | Done or in review % | Weighted % |\n|---|---|---|---|---|---|---|---|---|---|---|\n"
def line(label, t):
    return f"| {label} | {t['n']} | {t['done']} | {t['partial']} | {t['in review']} | {t['not started']} | {t['changed']} | {t['dropped']} | {t['p_done']} | {t['p_dr']} | {t['p_w']} |\n"

ids = list(inv)
COV_MD = os.path.join(WT, "docs/inventory/coverage.md")
prev = {}
if PREV:
    for l in io.open(COV_MD, encoding="utf-8"):
        if re.match(r"^\| [A-Z]+-\d+ \|", l):
            c = [x.strip() for x in l.strip()[2:-2].split(" | ")]
            prev[c[0]] = c[4]
    assert set(prev) == set(ids), "previous coverage.md does not hold the same IDs"
MS = {"M1": "Core and CLI: workspaces, terminals, git", "M2": "Headless server driven by the CLI", "M3": "Agents, status, delegation, credentials", "M4": "Desktop app, first-version scope", "M5": "Android app", "M6": "Extensions and the rest of the inventory"}

def changed_section():
    if not PREV: return ""
    s = f"## Changed since {PREV}\n\nThe previous measurement was taken on commit `{PREV}`. Same method, same reviewers' rules; every status change below was checked against the code on `{COMMIT}`.\n\n"
    s += f"| Scope | Done % ({PREV}) | Done % ({COMMIT}) | Done or in review % ({PREV}) | Done or in review % ({COMMIT}) | Weighted % ({PREV}) | Weighted % ({COMMIT}) |\n|---|---|---|---|---|---|---|\n"
    scopes = [("Whole inventory", ids), ("First version", [i for i in ids if i in fv])]
    scopes += [(m, [i for i in ids if milestone(i) == m]) for m in MS]
    for label, sel in scopes:
        if not sel: continue
        a, b = tally(sel, prev), tally(sel)
        s += f"| {label} | {a['p_done']} | {b['p_done']} | {a['p_dr']} | {b['p_dr']} | {a['p_w']} | {b['p_w']} |\n"
    moves = OrderedDict()
    for i in ids:
        if prev[i] != cov[i][1]: moves.setdefault((prev[i], cov[i][1]), []).append(i)
    s += f"\n{sum(len(v) for v in moves.values())} features changed status:\n\n| From | To | Features | IDs |\n|---|---|---|---|\n"
    for (a, b), v in sorted(moves.items(), key=lambda kv: (-len(kv[1]), kv[0])):
        s += f"| {a} | {b} | {len(v)} | {', '.join(v)} |\n"
    return s

out = io.StringIO()
tot = tally(ids); ftot = tally([i for i in ids if i in fv])
TODAY = datetime.date.today().isoformat()
out.write(f"""# Inventory coverage

Status: measured {TODAY} on commit `{COMMIT}` (`origin/main`), against the 542 features of the [functional inventory](README.md). Generated content: regenerate it rather than editing rows by hand (see Method).

## Headline

| | Features | Done % | Done or in review % | Weighted % | Done incl. built replacements % |
|---|---|---|---|---|---|
| Whole inventory | {tot['n']} | {tot['p_done']} | {tot['p_dr']} | {tot['p_w']} | {tot['p_cb']} |
| [First version](first-version.md) | {ftot['n']} | {ftot['p_done']} | {ftot['p_dr']} | {ftot['p_w']} | {ftot['p_cb']} |

Percentages exclude features marked `changed` or `dropped` from the denominator. **Weighted %** counts `done` as 1 and `partial` as 0.5. **Done or in review %** adds features held only in an open pull request. **Done incl. built replacements %** also counts as done the `changed` features whose replacement design is built on `main` ({tot['cb']} overall, {ftot['cb']} in the first version; detected from the row note, which says "built" and not "not built" or "in review").

{READING}
{changed_section()}
## Statuses

- `done`: reachable through the CLI or the desktop app against the real service on `main`. Evidence: the command or protocol method, and one file that implements it.
- `partial`: some layer exists on `main` (for example the service method without a CLI or UI, a UI on sample data only, or only part of the feature's rules). The note says what is missing.
- `in review`: the feature, or the layer that completes it, exists only in an open pull request (number given).
- `not started`: nothing on `main` or in an open pull request.
- `changed`: a decision record or owner direction replaced the feature with a different design; the note says whether the replacement is built.
- `dropped`: a decision record or owner direction removed it.

## Totals

### Overall and first version

{HDR}{line('Whole inventory', tot)}{line('First version', ftot)}
### Per inventory area

{HDR}""")
for p, (name, doc) in domains.items():
    out.write(line(f"[{name}]({doc}) ({p})", tally([i for i in ids if i.split('-')[0] == p])))
out.write(f"\n### Per milestone\n\nMilestones as defined in `docs/WORKING-RULES.md`. M1 to M5 hold first-version features only; everything outside the first version is M6.\n\n{HDR}")
for m, d in MS.items():
    sel = [i for i in ids if milestone(i) == m]
    if sel: out.write(line(f"{m} {d}", tally(sel)))
    else: out.write(f"| {m} {d} | 0 | - | - | - | - | - | - | - | - | - |\n")
out.write("""
## Method

1. Worktree at the measured commit; the open pull requests listed by `gh pr list --state open` at the same time.
2. Eight read-only reviewers, one per group of inventory areas, read the code (`crates/`), the CLI reference (`docs/cli.md`), the decision records (`docs/decisions/`) and the open pull request diffs, and gave each feature one status with evidence. Evidence or it is not done: a name alone was not enough, the reviewer had to open the implementing code; a screen drawn on sample data was not counted as done.
3. A script (`docs/inventory/coverage_assemble.py`) joined the rows with the inventory's README table and `first-version.md`, checked that every one of the 542 IDs appears exactly once, and computed the totals.
4. Milestone mapping (judgement, applied by prefix with listed exceptions): first-version features of WS, TERM and GIT, the core store (SET-01 to SET-04), `orca.yaml` scripts (CUS-01), core extension points (CUS-08), the local CLI groups (CLI-01, 03, 08, 09, 10, 11, 35) and core service hygiene (PLAT-07, PLAT-14, PLAT-18, PLAT-27) are M1; SRV, REM, CLD and CLI-04, CLI-05 are M2; AG, STAT, ACC, HIST, AUTO, profile import (SET-08), per-agent overrides (CUS-04) and the agent, automation and Bot Teams CLI groups (CLI-12, 14, 23, 24, 25, 32) are M3; every other first-version feature is M4. The inventory describes core behaviour, so no feature row is Android-only and M5 has no rows: Android reaches the M1 to M4 features through the same service.
5. To regenerate: repeat step 2 on a new commit, starting from the previous rows and re-checking every row the new commits could change, then run `python docs/inventory/coverage_assemble.py <repo-root> <rows-dir> <commit> <previous-commit>`; the last argument reads the existing `coverage.md` as the earlier measurement and writes the \"Changed since\" section. `<rows-dir>` may hold a `reading.txt` with the paragraph shown under the headline. Keep this method section in step with any change.

Limits: nothing was run; statuses come from reading code. The milestone mapping and the `partial` judgement involve reading the feature's intent, so two reviewers may differ on borderline rows. Work on local branches without an open pull request counts as not started.

## Features

| ID | Feature | Proposed | Milestone | Status | Evidence | Note |
|---|---|---|---|---|---|---|
""")
for i in ids:
    c = cov[i]
    out.write(f"| {i} | {inv[i]['name']} | {inv[i]['mark']} | {milestone(i)} | {c[1]} | {c[2]} | {c[3]} |\n")
io.open(os.path.join(WT, "docs/inventory/coverage.md"), "w", encoding="utf-8", newline="\n").write(out.getvalue())
print("TOTAL", dict(tot)); print("FV", dict(ftot))
for m in MS:
    sel = [i for i in ids if milestone(i) == m]; t = tally(sel) if sel else {}
    print(m, len(sel), t.get("p_done"), t.get("p_w"))
