let base = "http://127.0.0.1:9222";
let expression = "document.title";
const arg = process.argv[2];
if (arg && /^https?:\/\//.test(arg)) {
  base = arg;
  expression = process.argv[3] ?? "document.title";
} else if (arg) {
  expression = arg;
}
const list = await (await fetch(`${base}/json/list`)).json();
const page = list.find((t: any) => t.type === "page") ?? list[0];
if (!page?.webSocketDebuggerUrl) {
  throw new Error(`no page target: ${JSON.stringify(list)}`);
}
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise<void>((resolve, reject) => {
  ws.onopen = () => resolve();
  ws.onerror = (e) => reject(e);
});
ws.send(JSON.stringify({ id: 1, method: "Runtime.evaluate", params: { expression, returnByValue: true } }));
const msg = await new Promise<string>((resolve, reject) => {
  ws.onmessage = (e) => resolve(String(e.data));
  ws.onerror = (e) => reject(e);
  setTimeout(() => reject(new Error("timeout")), 30000);
});
const data = JSON.parse(msg);
const val = data?.result?.result?.value;
if (val !== undefined) console.log(typeof val === "string" ? val : JSON.stringify(val));
else console.log(JSON.stringify(data));
ws.close();
