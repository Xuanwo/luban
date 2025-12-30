import { Codex } from "@openai/codex-sdk";

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
    process.stdout.write(JSON.stringify(event) + "\n");
  }
}

main().catch((err) => {
  process.stderr.write(String(err?.stack ?? err) + "\n");
  process.exit(1);
});
