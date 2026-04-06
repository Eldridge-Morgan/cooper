/**
 * Cooper Test Utilities
 *
 * Provides helpers for testing Cooper apps with real infrastructure.
 * Used with `cooper test` which starts embedded Postgres, NATS, and Valkey.
 *
 * ```ts
 * import { testClient } from "cooper-stack/test";
 * import { describe, it } from "node:test";
 * import assert from "node:assert";
 *
 * describe("users API", () => {
 *   it("creates a user", async () => {
 *     const app = testClient();
 *     const res = await app.post("/api/users", { name: "Alice" });
 *     assert.strictEqual(res.status, 200);
 *     assert.strictEqual(res.body.name, "Alice");
 *   });
 * });
 * ```
 */

export interface TestResponse {
  status: number;
  headers: Record<string, string>;
  body: any;
}

export interface TestClientOptions {
  baseUrl?: string;
  headers?: Record<string, string>;
}

export interface TestClient {
  get(path: string, opts?: { headers?: Record<string, string> }): Promise<TestResponse>;
  post(path: string, body?: any, opts?: { headers?: Record<string, string> }): Promise<TestResponse>;
  put(path: string, body?: any, opts?: { headers?: Record<string, string> }): Promise<TestResponse>;
  patch(path: string, body?: any, opts?: { headers?: Record<string, string> }): Promise<TestResponse>;
  delete(path: string, opts?: { headers?: Record<string, string> }): Promise<TestResponse>;
  /** Set a default header for all subsequent requests */
  setHeader(key: string, value: string): void;
  /** Set Authorization: Bearer <token> header */
  setToken(token: string): void;
}

/**
 * Create a test client for making HTTP requests to the Cooper app.
 *
 * The base URL defaults to the COOPER_TEST_URL env var, or http://localhost:4000.
 */
export function testClient(opts?: TestClientOptions): TestClient {
  const baseUrl = opts?.baseUrl ?? process.env.COOPER_TEST_URL ?? "http://localhost:4000";
  const defaultHeaders: Record<string, string> = {
    "content-type": "application/json",
    ...opts?.headers,
  };

  async function request(
    method: string,
    path: string,
    body?: any,
    extraHeaders?: Record<string, string>
  ): Promise<TestResponse> {
    const url = `${baseUrl}${path}`;
    const headers = { ...defaultHeaders, ...extraHeaders };

    const res = await fetch(url, {
      method,
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });

    const contentType = res.headers.get("content-type") ?? "";
    let responseBody: any;
    if (contentType.includes("application/json")) {
      responseBody = await res.json();
    } else {
      responseBody = await res.text();
    }

    const responseHeaders: Record<string, string> = {};
    res.headers.forEach((value, key) => {
      responseHeaders[key] = value;
    });

    return {
      status: res.status,
      headers: responseHeaders,
      body: responseBody,
    };
  }

  return {
    get: (path, opts) => request("GET", path, undefined, opts?.headers),
    post: (path, body, opts) => request("POST", path, body, opts?.headers),
    put: (path, body, opts) => request("PUT", path, body, opts?.headers),
    patch: (path, body, opts) => request("PATCH", path, body, opts?.headers),
    delete: (path, opts) => request("DELETE", path, undefined, opts?.headers),
    setHeader(key: string, value: string) {
      defaultHeaders[key] = value;
    },
    setToken(token: string) {
      defaultHeaders["authorization"] = `Bearer ${token}`;
    },
  };
}

/**
 * Get a database client for direct DB access in tests.
 * Uses the same connection URLs injected by `cooper test`.
 */
export { database } from "./db.js";

/**
 * Get a cache client for direct cache access in tests.
 */
export { cache } from "./cache.js";
