import http from "node:http";
import next from "next";
import httpProxy from "http-proxy";
import { fileURLToPath } from "node:url";
import path from "node:path";

const port = Number(process.env.PORT || 3000);
const hostname = "0.0.0.0";
const internalApiOrigin = process.env.INTERNAL_API_ORIGIN || "http://127.0.0.1:3001";
const internalWsOrigin = process.env.INTERNAL_WS_ORIGIN || "ws://127.0.0.1:3001";
const appDir = path.dirname(fileURLToPath(import.meta.url));

const app = next({ dev: false, dir: appDir, hostname, port });
const handle = app.getRequestHandler();

const proxy = httpProxy.createProxyServer({
  changeOrigin: true,
  xfwd: true,
});

proxy.on("error", (error, req, res) => {
  const message = error instanceof Error ? error.message : "Unknown proxy error";

  if (res && "writeHead" in res) {
    const response = res;
    response.writeHead(502, { "Content-Type": "application/json" });
    response.end(JSON.stringify({ error: message }));
    return;
  }

  if (res && "destroy" in res) {
    res.destroy();
  }
});

function stripApiPrefix(url = "/") {
  if (url === "/api") return "/";
  if (url.startsWith("/api/")) return url.slice(4);
  return url;
}

await app.prepare();

const server = http.createServer((req, res) => {
  if (req.url?.startsWith("/api")) {
    req.url = stripApiPrefix(req.url);
    proxy.web(req, res, { target: internalApiOrigin });
    return;
  }

  handle(req, res);
});

server.on("upgrade", (req, socket, head) => {
  if (req.url?.startsWith("/ws/")) {
    proxy.ws(req, socket, head, { target: internalWsOrigin });
    return;
  }

  socket.destroy();
});

server.listen(port, hostname, () => {
  console.log(`Bundled frontend listening on http://${hostname}:${port}`);
});
