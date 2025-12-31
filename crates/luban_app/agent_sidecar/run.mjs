import { Codex } from "@openai/codex-sdk";

const EVENT_PREFIX = "__LUBAN_EVENT__ ";

function redirectConsoleToStderr() {
  const write = (args) => {
    try {
      const msg = args
        .map((a) =>
          typeof a === "string" ? a : JSON.stringify(a, null, 2),
        )
        .join(" ");
      process.stderr.write(msg + "\n");
    } catch {
      process.stderr.write(String(args) + "\n");
    }
  };

  console.log = (...args) => write(args);
  console.info = (...args) => write(args);
  console.warn = (...args) => write(args);
  console.error = (...args) => write(args);

  process.on("warning", (warning) => {
    process.stderr.write(String(warning?.stack ?? warning) + "\n");
  });
}

function readAllStdin() {
  return new Promise((resolve, reject) => {
    let data = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => (data += chunk));
    process.stdin.on("end", () => resolve(data));
    process.stdin.on("error", reject);
  });
}

async function main() {
  redirectConsoleToStderr();

  const raw = await readAllStdin();
  const req = JSON.parse(raw);

  const codex = new Codex();
  const threadOptions = {
    workingDirectory: req.workingDirectory,
    sandboxMode: req.sandboxMode,
    approvalPolicy: req.approvalPolicy,
    networkAccessEnabled: req.networkAccessEnabled,
    webSearchEnabled: req.webSearchEnabled,
    skipGitRepoCheck: req.skipGitRepoCheck,
  };

  const thread = req.threadId
    ? codex.resumeThread(req.threadId, threadOptions)
    : codex.startThread(threadOptions);

  const { events } = await thread.runStreamed(req.prompt);
  for await (const event of events) {
    process.stdout.write(EVENT_PREFIX + JSON.stringify(event) + "\n");
  }
}

main().catch((err) => {
  process.stderr.write(String(err?.stack ?? err) + "\n");
  process.exit(1);
});
