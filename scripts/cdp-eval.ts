let base = "http://127.0.0.1:9222";
let expression = "document.title";
let method: string | null = null;
let params: any = null;

const argv = process.argv.slice(2);
let i = 0;
if (argv[i] && /^https?:\/\//.test(argv[i])) {
  base = argv[i++];
}
while (i < argv.length) {
  if (argv[i] === "--method") {
    method = argv[++i];
    i++;
  } else if (argv[i] === "--params") {
    params = JSON.parse(argv[++i] ?? "{}");
    i++;
  } else if (!method && expression === "document.title") {
    expression = argv[i++];
  } else {
    i++;
  }
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

if (method) {
  ws.send(JSON.stringify({ id: 1, method, params: params ?? {} }));
} else {
  ws.send(JSON.stringify({ id: 1, method: "Runtime.evaluate", params: { expression, returnByValue: true } }));
}

const msg = await new Promise<string>((resolve, reject) => {
  ws.onmessage = (e) => resolve(String(e.data));
  ws.onerror = (e) => reject(e);
  setTimeout(() => reject(new Error("timeout")), 30000);
});
const data = JSON.parse(msg);
if (method) {
  console.log(JSON.stringify(data.result ?? data, null, 2));
} else {
  const val = data?.result?.result?.value;
  if (val !== undefined) console.log(typeof val === "string" ? val : JSON.stringify(val));
  else console.log(JSON.stringify(data));
}
ws.close();

