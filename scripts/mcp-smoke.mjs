// Optional native smoke test. Only operates on an explicitly named synthetic QA conversation.
// node scripts/mcp-smoke.mjs /path/to/patter /path/to/agent.sock off|read|edit|process
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import assert from "node:assert/strict";
const [binary, socket, mode = "read"] = process.argv.slice(2);
assert(binary && socket, "Supply the executable and private socket paths.");
const child = spawn(binary, ["--mcp", "--socket", socket], {
  stdio: ["pipe", "pipe", "inherit"],
});
const pending = new Map();
let seq = 0;
createInterface({ input: child.stdout }).on("line", (line) => {
  const msg = JSON.parse(line),
    entry = pending.get(msg.id);
  if (entry) {
    clearTimeout(entry.timer);
    pending.delete(msg.id);
    msg.error
      ? entry.reject(new Error(JSON.stringify(msg.error)))
      : entry.resolve(msg.result);
  }
});
child.on("exit", (code) => {
  for (const entry of pending.values()) {
    clearTimeout(entry.timer);
    entry.reject(new Error(`MCP exited ${code}`));
  }
  pending.clear();
});
function request(method, params) {
  return new Promise((resolve, reject) => {
    const id = ++seq;
    const timer = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`${method} timed out`));
    }, 20000);
    pending.set(id, { resolve, reject, timer });
    child.stdin.write(
      JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n",
    );
  });
}
async function call(name, args = {}) {
  const r = await request("tools/call", { name, arguments: args });
  return {
    error: r.isError === true,
    value: r.isError ? r.content[0].text : JSON.parse(r.content[0].text),
  };
}
try {
  const hello = await request("initialize", {
    protocolVersion: "2025-11-25",
    capabilities: {},
    clientInfo: { name: "patter-qa", version: "1" },
  });
  assert.equal(hello.serverInfo.name, "patter");
  child.stdin.write(
    JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) +
      "\n",
  );
  const listed = await request("tools/list", {});
  assert.equal(listed.tools.length, 9);
  const search = await call("search_conversations", { query: "MCP QA" });
  if (mode === "off") {
    assert(search.error);
    assert.match(search.value, /off|not running|Open Patter/);
    console.log(
      "PASS: MCP initialized, nine tools discovered, access refused.",
    );
  } else {
    assert(!search.error, JSON.stringify(search));
    const m = search.value.conversations.find((m) =>
      m.title.startsWith("MCP QA"),
    );
    assert(
      m,
      "Create a synthetic conversation titled MCP QA before this test.",
    );
    const read = await call("get_conversation", {
      id: m.id,
      section: "transcript",
      limit: 1,
    });
    assert(!read.error);
    assert(read.value.content.length === 1);
    if (mode === "read") {
      const denied = await call("edit_conversation", {
        id: m.id,
        expected_revision: m.revision,
        notes: "Must not save",
      });
      assert(denied.error);
      assert.match(denied.value, /Editing is off/);
      const deniedJob = await call("start_processing", {
        id: m.id,
        expected_revision: m.revision,
        kind: "summary",
      });
      assert(deniedJob.error);
      assert.match(deniedJob.value, /processing is off/);
      console.log(
        "PASS: timestamped transcript read; edits and processing denied.",
      );
    } else if (mode === "edit") {
      const changed = await call("edit_conversation", {
        id: m.id,
        expected_revision: m.revision,
        notes: "MCP QA: agent note saved through the app.",
      });
      assert(!changed.error, JSON.stringify(changed));
      const stale = await call("edit_conversation", {
        id: m.id,
        expected_revision: m.revision,
        title: "Must not overwrite",
      });
      assert(stale.error);
      assert.match(stale.value, /REVISION_CONFLICT/);
      const opened = await call("open_conversation", { id: m.id });
      assert(!opened.error);
      const history = await call("conversation_history", { id: m.id });
      assert(!history.error);
      assert(history.value.versions.length >= 2);
      console.log(
        "PASS: edit saved, stale write refused, history retained, UI open requested.",
      );
    } else if (mode === "process") {
      const started = await call("start_processing", {
        id: m.id,
        expected_revision: m.revision,
        kind: "summary",
        template: "interview",
      });
      assert(!started.error, JSON.stringify(started));
      const duplicate = await call("start_processing", {
        id: m.id,
        expected_revision: m.revision,
        kind: "summary",
      });
      assert(duplicate.error);
      assert.match(duplicate.value, /already running|REVISION_CONFLICT/);
      let job;
      for (let i = 0; i < 30; i++) {
        job = await call("job_status", { id: started.value.jobId });
        assert(!job.error);
        if (job.value.status !== "running") break;
        await new Promise((r) => setTimeout(r, 200));
      }
      assert.equal(job.value.status, "completed", JSON.stringify(job));
      const summary = await call("get_conversation", {
        id: m.id,
        section: "summary",
      });
      assert(!summary.error);
      assert.match(summary.value.content, /QA/);
      console.log(
        "PASS: local summary job completed and saved with the requested template.",
      );
    } else throw new Error("Unknown mode");
  }
} finally {
  child.stdin.end();
  child.kill();
}
