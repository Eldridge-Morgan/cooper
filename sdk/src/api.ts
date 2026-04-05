import { registry, type MiddlewareFn } from "./registry.js";
import { CooperError } from "./error.js";

export interface ApiConfig {
  method?: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  path: string;
  auth?: boolean;
  stream?: "sse" | "websocket";
  validate?: any; // Zod schema
  middleware?: MiddlewareFn[];
}

/**
 * Define an API endpoint.
 *
 * ```ts
 * export const getUser = api(
 *   { method: "GET", path: "/users/:id", auth: true },
 *   async ({ id }, principal) => {
 *     const user = await db.queryRow("SELECT * FROM users WHERE id = $1", [id]);
 *     if (!user) throw new CooperError("NOT_FOUND", "User not found");
 *     return { user };
 *   }
 * );
 * ```
 */
export function api<TInput = any, TOutput = any>(
  config: ApiConfig,
  handler: (input: TInput, principal?: any) => Promise<TOutput>
): { _cooper_type: "api"; config: ApiConfig; handler: typeof handler } {
  const method = config.method ?? "GET";
  const key = `${method}:${config.path}`;

  const descriptor = {
    _cooper_type: "api" as const,
    config,
    handler,
  };

  // Register for the bridge to find at runtime
  registry.registerRoute(key, {
    method,
    path: config.path,
    auth: config.auth ?? false,
    stream: config.stream,
    validate: config.validate,
    middleware: config.middleware ?? [],
    handler,
    exportName: "", // filled in by bridge during module scan
    sourceFile: "",
  });

  return descriptor;
}

export { CooperError };
