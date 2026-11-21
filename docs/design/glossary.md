# MassOrca design glossary

Agreed names for the desktop regions and core concepts. Use these terms in every design document, in the code and when talking about the product. Owner-approved on 2026-11-12.

## Desktop window regions

Left to right, inside one window:

| Term | Spanish | What it holds | Notes |
|---|---|---|---|
| Activity rail | Barra de actividad | Mode switch (Workspaces / Bot Teams), Tasks, Calendar, Marketplace, theme toggle, per-agent usage rings, Settings | Always visible. Never hidden by the sidebar toggle. |
| Sidebar | Barra lateral | The workspaces tree in Workspaces mode, the teams tree in Bot Teams mode | Can be hidden with the sidebar toggle. |
| Title bar | Barra de título | Sidebar toggle, navigation, session title, region toggles, window controls | |
| Frame | Marco | Activity rail + sidebar + title bar, drawn as one surface with no seams | `surface.frame`: #1b1b1c in dark. |
| Canvas | Lienzo | Everything inset in the frame, with a rounded top-left corner | `surface.canvas`: #151517 in dark. |
| Conversation | Conversación | The chat of a session or a Conference room, its message minimap and the composer | The central region. |
| Session summary | Resumen de sesión | A column of summary cards for the session | Only cards that have information are shown. A card appears when its first record arrives. |
| Work panel | Panel de trabajo | Tabs for Terminal, Browser, Markdown or file, and Mobile emulator | Empty state offers one action per tab type. |
| Project sidebar | Barra del proyecto | File explorer, Sessions list, Source control (changes), Checks | The rightmost region. |
| Status bar | Barra de estado | Agent status, memory use, terminal count, ports, hosts with connection state | A strip at the bottom. |

## Modes

| Term | Spanish | Meaning |
|---|---|---|
| Workspaces mode | Modo Workspaces | For software engineers: coding work in git worktrees or plain folders, with coding agents and full options. |
| Bot Teams mode | Modo Bot Teams | For people who are not necessarily technical: teams of bots that work from a Conference room, with simpler controls. |
| Workspaces in a team | Workspaces dentro de un team | An option a team can turn on (for example a Development team) to get Workspaces depth inside Bot Teams. |

## Bot Teams

| Term | Spanish | Meaning |
|---|---|---|
| Conference room | Conference room | The chat with a team's bots. A requirement detected there becomes a task, and a request to schedule work becomes a routine. |
| Board | Board | The team's tasks, generated from the Conference room. |
| Routine | Rutina | The simplified, Bot Teams form of an automation: a task that runs on a schedule. Created from the Routines page or by a bot in the Conference room. Shown on the Calendar. |
| Automation | Automatización | The full, Workspaces form of a scheduled or triggered run. Same core engine as a routine. Shown on the Calendar. |
| Web address | Dirección web | A webhook that another app calls to start one task. |
| Lead bot | Bot responsable | The bot that owns a composite requirement and delegates parts to other bots. |

## Shared concepts

| Term | Spanish | Meaning |
|---|---|---|
| Delegation | Delegación | One core engine. A lead bot or a coordinator coding agent hands parts of the work to child bots or child agents. Shown as a delegation tree. |
| Model picker | Selector de modelo | The one component that picks agent, model, reasoning effort, speed and access in every composer. |
| Secret vault | Vault de secretos | Stores credentials for agents. Values never enter the chat. Scopes: project (default), team, global. Sharing creates a link, not a copy. |
| Extension | Extensión | A package that adds panels, commands, views, rail icons, status items or settings sections. |
| Creator mode | Modo creador | A session mode in which the agent builds or changes an extension in its own folder, with a live Preview, Files and Logs in the Work panel. |
| Marketplace | Marketplace | The catalogue of installable apps, agents, skills and services. Opened from the Activity rail. |
| Child session | Sesión hija | A session started by a coordinator agent or a lead bot to do part of the work. Nested under its parent. A separate session: its own Sidebar row, its own worktree by default, any agent. Shown in the "Child sessions" summary card. Never called a subagent. |
| Subagent | Subagente | A helper an agent runs inside its own session (for example Claude's Explore). It has no session, no Sidebar row and no worktree of its own, and it is shown only in the "Subagents" summary card of the session that ran it. Not a child session. |
| Own worktree | Worktree propio | Where a child session runs by default: a new git worktree and branch of its own. Always used for children launched in parallel. |
| Current worktree | Worktree actual | Running a child in the parent's worktree. Allowed only for read-only children, plain-folder workspaces, or a manual pick on the delegation card. |
| Parent waiting | Sesión padre en espera | The state of a parent session while a child writes in the current worktree: the parent's edits pause until the child finishes. |
| Bring back | Traer de vuelta | Merging a child session's result into its parent session. |
| Checklist | Checklist | The list of steps an agent keeps for a session. Shown as a Session summary card. |
| Results | Resultados | The Session summary card that lists URLs, files and artifacts a session produced. |
| Artifact | Artifact | A page or file an agent produced in a session. Listed in Results and opened in a Work panel tab. |
| Prototypes | Prototipos | Core Workspaces feature for building and comparing UI prototypes, with its own design system, directions and merge. |
| No Git | Sin Git | The marker on a workspace that is a plain folder, with an "Initialize Git" action. |
| Mention | Mención | An @ reference to a bot, file or session in a composer. |

Internal names that never appear in UI copy: "side panel" (code name of the Work panel), "right sidebar" (Project sidebar), "Chat summary" (Session summary), "Jev" (internal routing).
