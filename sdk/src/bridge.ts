#!/usr/bin/env node
/**
 * Cooper JS Worker Bridge
 *
 * Spawned by the Rust runtime as a subprocess. Communicates via
 * JSON-RPC over stdin/stdout. Loads user TypeScript modules and
 * executes handler functions on demand.
 *
 * Protocol:
 *   → { "id": 1, "method": "call", "params": { "source": "services/users/api.ts", "export": "getUser", "input": {...}, "auth": {...} } }
 *   ← { "id": 1, "result": {...} }
 *   ← { "id": 1, "error": { "code": "NOT_FOUND", "message": "User not found" } }
 */

import { registry } from "./registry.js";
import { CooperError } from "./error.js";
import { createRequire } from "node:module";
import { createInterface } from "node:readline";
import path from "node:path";
import { pathToFileURL } from "node:url";

const projectRoot = process.env.COOPER_PROJECT_ROOT ?? process.cwd();

// Module cache — avoid re-importing on every request
const moduleCache = new Map<string, any>();

async function loadModule(sourcePath: string): Promise<any> {
  if (moduleCache.has(sourcePath)) return moduleCache.get(sourcePath)!;

  const fullPath = path.resolve(projectRoot, sourcePath);
  const fileUrl = pathToFileURL(fullPath).href;

  try {
    const mod = await import(fileUrl);
    moduleCache.set(sourcePath, mod);
    return mod;
  } catch (err: any) {
    throw new Error(`Failed to load module "${sourcePath}": ${err.message}`);
  }
}

interface RPCRequest {
  id: number;
  method: string;
  params: any;
}

interface RPCResponse {
  id: number;
  result?: any;
  error?: { code: string; message: string; statusCode?: number };
}

async function handleCall(params: {
  source: string;
  export: string;
  input: any;
  auth?: any;
  headers?: Record<string, string>;
}): Promise<any> {
  const mod = await loadModule(params.source);
  const exported = mod[params.export];

  if (!exported) {
    throw new CooperError("NOT_FOUND", `Export "${params.export}" not found in ${params.source}`);
  }

  // Get the handler function
  let handler: Function;
  let routeConfig: any = null;

  if (exported._cooper_type === "api") {
    handler = exported.handler;
    routeConfig = exported.config;
  } else if (typeof exported === "function") {
    handler = exported;
  } else {
    throw new CooperError("INTERNAL", `Export "${params.export}" is not callable`);
  }

  // Validation — run Zod schema if present.
  // Use passthrough() to preserve path params that aren't in the schema.
  let validatedInput = params.input;
  if (routeConfig?.validate) {
    const schema = routeConfig.validate;
    const lenient = typeof schema.passthrough === "function" ? schema.passthrough() : schema;
    const result = lenient.safeParse(params.input);
    if (!result.success) {
      throw new CooperError(
        "VALIDATION_FAILED",
        result.error.issues.map((i: any) => `${i.path.join(".")}: ${i.message}`).join("; ")
      );
    }
    validatedInput = result.data;
  }

  // Auth — verify token and inject principal
  let principal: any = undefined;
  if (routeConfig?.auth) {
    if (!params.auth?.token) {
      throw new CooperError("UNAUTHORIZED", "Authentication required");
    }
    if (registry.authHandler) {
      principal = await registry.authHandler(params.auth.token);
    } else {
      throw new CooperError("INTERNAL", "No auth handler registered");
    }
  }

  // Middleware chain
  const middlewares = [
    ...registry.globalMiddleware,
    ...(routeConfig?.middleware ?? []),
  ];

  const req = {
    ...validatedInput,
    headers: params.headers ?? {},
    ip: params.headers?.["x-forwarded-for"] ?? "127.0.0.1",
    method: routeConfig?.method ?? "GET",
    path: routeConfig?.path ?? "/",
  };

  // Build the middleware chain
  let idx = 0;
  const runMiddleware = async (currentReq: any): Promise<any> => {
    if (idx < middlewares.length) {
      const mw = middlewares[idx++];
      return mw(currentReq, runMiddleware);
    }
    // End of chain — call the actual handler
    if (routeConfig?.auth) {
      return handler(validatedInput, principal);
    }
    return handler(validatedInput);
  };

  return runMiddleware(req);
}

async function handleCron(params: { source: string; export: string }): Promise<any> {
  const mod = await loadModule(params.source);
  const exported = mod[params.export];

  if (!exported) {
    throw new CooperError("NOT_FOUND", `Cron "${params.export}" not found in ${params.source}`);
  }

  if (exported._cooper_type === "cron") {
    // The cron was registered via the SDK, find it in the registry
    const cronEntry = registry.crons.get(exported.name);
    if (cronEntry) {
      await cronEntry.handler();
      return { ok: true };
    }
  }

  throw new CooperError("INTERNAL", `Cannot execute cron "${params.export}"`);
}

async function handlePubSub(params: {
  topic: string;
  subscriber: string;
  data: any;
}): Promise<any> {
  const topicEntry = registry.topics.get(params.topic);
  if (!topicEntry) {
    throw new CooperError("NOT_FOUND", `Topic "${params.topic}" not registered`);
  }

  const sub = topicEntry.subscribers.get(params.subscriber);
  if (!sub) {
    throw new CooperError("NOT_FOUND", `Subscriber "${params.subscriber}" not found on topic "${params.topic}"`);
  }

  await sub.handler(params.data);
  return { ok: true };
}

async function handleRequest(req: RPCRequest): Promise<RPCResponse> {
  try {
    let result: any;

    switch (req.method) {
      case "call":
        result = await handleCall(req.params);
        break;
      case "cron":
        result = await handleCron(req.params);
        break;
      case "pubsub":
        result = await handlePubSub(req.params);
        break;
      case "ping":
        result = { pong: true, pid: process.pid };
        break;
      case "invalidate":
        // Clear module cache for hot reload
        moduleCache.clear();
        result = { ok: true };
        break;
      default:
        throw new CooperError("INVALID_ARGUMENT", `Unknown method: ${req.method}`);
    }

    return { id: req.id, result };
  } catch (err: any) {
    if (err instanceof CooperError) {
      return {
        id: req.id,
        error: { code: err.code, message: err.message, statusCode: err.statusCode },
      };
    }
    return {
      id: req.id,
      error: { code: "INTERNAL", message: err.message ?? "Unknown error", statusCode: 500 },
    };
  }
}

// Main loop — read JSON lines from stdin, write responses to stdout
const rl = createInterface({ input: process.stdin });

rl.on("line", async (line) => {
  if (!line.trim()) return;

  try {
    const req: RPCRequest = JSON.parse(line);
    const res = await handleRequest(req);
    process.stdout.write(JSON.stringify(res) + "\n");
  } catch (err: any) {
    process.stdout.write(
      JSON.stringify({ id: 0, error: { code: "INTERNAL", message: `Bridge parse error: ${err.message}` } }) + "\n"
    );
  }
});

// Preload auth handlers and middleware — these register via side effects
async function preloadSideEffects() {
  const fs = await import("node:fs");
  const servicesDir = path.join(projectRoot, "services");
  if (!fs.existsSync(servicesDir)) return;

  const scanDir = (dir: string) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const fullPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        scanDir(fullPath);
      } else if (entry.name.endsWith(".ts") || entry.name.endsWith(".js")) {
        try {
          const content = fs.readFileSync(fullPath, "utf-8");
          if (content.includes("authHandler") || content.includes("middleware(")) {
            const relative = path.relative(projectRoot, fullPath);
            loadModule(relative).catch(() => {});
          }
        } catch {}
      }
    }
  };

  scanDir(servicesDir);
}

await preloadSideEffects();

// Signal ready
process.stdout.write(JSON.stringify({ id: 0, result: { ready: true, pid: process.pid } }) + "\n");
