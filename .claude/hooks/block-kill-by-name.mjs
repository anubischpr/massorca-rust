// PreToolUse guard: several sessions share this machine, so a shell command that
// kills processes by name (or shuts down all of WSL) is refused. Stop only what
// you started, by PID. The main checkout, where `.git` is a directory, is exempt:
// that is where the orchestrating session runs.
import { readFileSync, statSync } from "node:fs";
import { join } from "node:path";

// Where a command name can start: the beginning, or after a separator, `sudo` or `xargs`.
// Keeps a word inside a quoted message (a commit text, an echo) from matching.
const AT = "(?:^|[|;&(`\\n]\\s*|\\b(?:sudo|xargs|exec|timeout\\s+\\S+)\\s+)";

const RULES = [
  [new RegExp(`${AT}taskkill(\\.exe)?\\b[^|;&\\n]*[\\s"'][/-]im\\b`, "i"), "taskkill /IM kills every process with that image name"],
  [new RegExp(`${AT}(pkill|killall)\\b`, "i"), "pkill and killall match processes by name"],
  [/\b(stop-process|spps|kill)\b[^|;&\n]*\s-(name|processname)\b/i, "Stop-Process -Name matches processes by name"],
  [/\bget-process\b(?![^|\n]*\s-id\b)[^|\n]*\|\s*(stop-process|spps|kill)\b/i, "Get-Process <name> | Stop-Process matches processes by name"],
  [new RegExp(`${AT}wmic\\b[^|;&\\n]*\\bprocess\\b[^|;&\\n]*\\b(delete|terminate)\\b`, "i"), "wmic process ... delete matches processes by name"],
  [new RegExp(`${AT}wsl(\\.exe)?\\b[^|;&\\n]*\\s--(shutdown|terminate)\\b`, "i"), "wsl --shutdown or --terminate stops other sessions' work in WSL"],
];

function isMainCheckout(cwd) {
  try {
    return statSync(join(cwd, ".git")).isDirectory();
  } catch {
    return false;
  }
}

let input;
try {
  input = JSON.parse(readFileSync(0, "utf8"));
} catch {
  process.exit(0);
}

const command = input?.tool_input?.command;
if (typeof command !== "string" || isMainCheckout(input?.cwd ?? process.cwd())) {
  process.exit(0);
}

const hit = RULES.find(([pattern]) => pattern.test(command));
if (!hit) {
  process.exit(0);
}

const reason =
  `Blocked: ${hit[1]}. Other sessions share this machine. ` +
  "Stop only processes you started, by PID (taskkill /PID <pid>, kill <pid>, Stop-Process -Id <pid>). " +
  "If something outside your session must be stopped, ask the orchestrating session.";
console.log(
  JSON.stringify({
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: reason,
    },
  }),
);
