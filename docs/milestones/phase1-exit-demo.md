# Phase 1 exit demo: workspaces from the CLI

The Phase 1 exit condition: the `massorca` CLI creates, lists, shows and removes workspaces
for both git worktrees and plain folders, against a local service, on Windows and Linux.

Both runs used a fresh temporary `MASSORCA_HOME`, a temporary git repository (`repo`, one
commit) and a temporary folder (`notes`, holding `notes.txt`). The workspaces root was passed
with `--workspaces-root`, so nothing was written to the user's home. Temporary paths appear as
`<tmp>`. The same shell script ran on both platforms against a `massorca` binary built from
this branch; the output below is verbatim apart from those path placeholders.

## Windows (Git Bash)

```text
warning: in the working copy of 'README.md', LF will be replaced by CRLF the next time Git touches it
$ git init repo && commit README.md   # in <tmp>/repo
$ massorca project add <tmp>/repo --workspaces-root <tmp>/ws
added project repo (git) at <tmp>\repo
[exit 0]
$ massorca project add <tmp>/notes --folder
added project notes (folder) at <tmp>\notes
[exit 0]
$ massorca workspace create repo feature
created workspace feature (worktree) at <tmp>\ws\feature
[exit 0]
$ massorca workspace create notes main
created workspace main (folder) at <tmp>\notes
[exit 0]
$ massorca workspace list
ID                                    NAME     KIND      BRANCH   PATH                                                         STATE
01a1fcf5-c7d5-73e5-8082-9f845251e33a  feature  worktree  feature  <tmp>\ws\feature  ready
01a1fcf5-c80e-772f-b0ac-ea878399fa0c  main     folder    -        <tmp>\notes       ready
[exit 0]
$ massorca workspace show feature
id: 01a1fcf5-c7d5-73e5-8082-9f845251e33a
name: feature
project: 01a1fcf5-c50e-70ba-a2c5-b3f37c15e7dc
kind: worktree
branch: feature
base: HEAD
path: <tmp>\ws\feature
state: ready
[exit 0]
$ massorca workspace rm feature
removed workspace feature
deleted branch feature
[exit 0]
$ massorca workspace rm main
removed workspace main
kept directory <tmp>\notes
[exit 0]
$ massorca workspace list
no workspaces
[exit 0]
$ massorca service stop
service stopped
[exit 0]
$ ls <tmp>/notes   # the folder workspace's files stay
notes.txt
```

## Linux (WSL, Ubuntu 24.04)

```text
$ git init repo && commit README.md   # in <tmp>/repo
$ massorca project add /repo --workspaces-root /ws
added project repo (git) at /repo
[exit 0]
$ massorca project add /notes --folder
added project notes (folder) at /notes
[exit 0]
$ massorca workspace create repo feature
created workspace feature (worktree) at /ws/feature
[exit 0]
$ massorca workspace create notes main
created workspace main (folder) at /notes
[exit 0]
$ massorca workspace list
ID                                    NAME     KIND      BRANCH   PATH         STATE
01a1fcf6-712e-7263-9de1-6267eee9c27b  feature  worktree  feature  /ws/feature  ready
01a1fcf6-7134-72b1-a1a1-ae24d9c990d0  main     folder    -        /notes       ready
[exit 0]
$ massorca workspace show feature
id: 01a1fcf6-712e-7263-9de1-6267eee9c27b
name: feature
project: 01a1fcf6-70dc-77d9-a3c3-1a32fef0b4e5
kind: worktree
branch: feature
base: HEAD
path: /ws/feature
state: ready
[exit 0]
$ massorca workspace rm feature
removed workspace feature
deleted branch feature
[exit 0]
$ massorca workspace rm main
removed workspace main
kept directory /notes
[exit 0]
$ massorca workspace list
no workspaces
[exit 0]
$ massorca service stop
service stopped
[exit 0]
$ ls <tmp>/notes   # the folder workspace's files stay
notes.txt
```
