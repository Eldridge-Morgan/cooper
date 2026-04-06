# Testing

Cooper provides a built-in test harness that spins up real infrastructure for your tests.

## Running tests

```bash
cooper test
```

This command:
1. Starts embedded Postgres, NATS, and Valkey
2. Runs your database migrations
3. Discovers and executes all `**/*.test.ts` files
4. Tears down infrastructure after tests complete
5. Exits with code 0 (pass) or 1 (fail)

## Writing tests

Cooper uses Node.js built-in test runner (Node 20+):

```ts
// services/users/api.test.ts
import { describe, it, beforeEach } from "node:test";
import assert from "node:assert";
import { testClient, database } from "cooper-stack/test";

const app = testClient();
const db = database("main");

beforeEach(async () => {
  await db.exec("DELETE FROM users");
});

describe("POST /users", () => {
  it("creates a user", async () => {
    const res = await app.post("/users", {
      name: "Alice",
      email: "alice@test.com",
    });

    assert.strictEqual(res.status, 200);
    assert.strictEqual(res.body.user.name, "Alice");
    assert.strictEqual(res.body.user.email, "alice@test.com");
    assert.ok(res.body.user.id);
  });

  it("returns validation error for missing fields", async () => {
    const res = await app.post("/users", {});
    assert.strictEqual(res.status, 422);
  });
});

describe("GET /users", () => {
  it("lists all users", async () => {
    await app.post("/users", { name: "Alice", email: "a@test.com" });
    await app.post("/users", { name: "Bob", email: "b@test.com" });

    const res = await app.get("/users");
    assert.strictEqual(res.status, 200);
    assert.strictEqual(res.body.users.length, 2);
  });
});
```

## Test client API

```ts
import { testClient } from "cooper-stack/test";

const app = testClient();
// or with custom base URL
const app = testClient({ baseUrl: "http://localhost:4100" });
```

### HTTP methods

```ts
const res = await app.get("/users");
const res = await app.post("/users", { name: "Alice" });
const res = await app.put("/users/123", { name: "Bob" });
const res = await app.patch("/users/123", { name: "Charlie" });
const res = await app.delete("/users/123");
```

### Response shape

```ts
interface TestResponse {
  status: number;                    // HTTP status code
  headers: Record<string, string>;   // Response headers
  body: any;                         // Parsed JSON or raw text
}
```

### Authentication

```ts
app.setToken("eyJhbGciOiJIUzI1NiIs...");

// All subsequent requests include Authorization: Bearer <token>
const res = await app.get("/protected/resource");
```

### Custom headers

```ts
app.setHeader("x-api-key", "my-key");

// Or per-request
const res = await app.get("/users", {
  headers: { "x-custom": "value" },
});
```

## Direct database access

Access the database directly in tests for setup and assertions:

```ts
import { database } from "cooper-stack/test";

const db = database("main");

// Setup
await db.exec("INSERT INTO users (name, email) VALUES ($1, $2)", ["Alice", "a@test.com"]);

// Assert
const user = await db.queryRow("SELECT * FROM users WHERE email = $1", ["a@test.com"]);
assert.strictEqual(user.name, "Alice");
```

## Cache access

```ts
import { cache } from "cooper-stack/test";

const userCache = cache("users");

// Pre-populate cache
await userCache.set("user:123", { name: "Alice" });

// Assert cache state
const cached = await userCache.get("user:123");
assert.deepStrictEqual(cached, { name: "Alice" });
```

## Filtering tests

Run only tests matching a pattern:

```bash
cooper test --filter users     # runs **/*users*.test.ts
cooper test --filter auth      # runs **/*auth*.test.ts
```

## Fail fast

Stop on first failure:

```bash
cooper test --fail-fast
```

## CI integration

`cooper test` returns exit code 0 on success, 1 on failure. Works with any CI:

```yaml
# GitHub Actions
- name: Run tests
  run: cooper test
```

## Test patterns

### Testing with transactions

Each test can use transactions for isolation:

```ts
import { database } from "cooper-stack/test";

const db = database("main");

it("transfers money atomically", async () => {
  await db.exec("INSERT INTO accounts (id, balance) VALUES (1, 1000), (2, 500)");

  const res = await app.post("/transfer", { from: 1, to: 2, amount: 200 });
  assert.strictEqual(res.status, 200);

  const from = await db.queryRow("SELECT balance FROM accounts WHERE id = 1");
  const to = await db.queryRow("SELECT balance FROM accounts WHERE id = 2");
  assert.strictEqual(from.balance, 800);
  assert.strictEqual(to.balance, 700);
});
```

### Testing rate limits

```ts
it("rate limits after 5 requests", async () => {
  for (let i = 0; i < 5; i++) {
    const res = await app.post("/auth/login", { email: "a@b.com", password: "123" });
    assert.strictEqual(res.status, 200);
  }

  const res = await app.post("/auth/login", { email: "a@b.com", password: "123" });
  assert.strictEqual(res.status, 429);
  assert.ok(res.headers["retry-after"]);
});
```

### Testing pub/sub side effects

```ts
it("publishes event on user creation", async () => {
  const res = await app.post("/users", { name: "Alice", email: "a@test.com" });
  assert.strictEqual(res.status, 200);

  // Give subscriber time to process
  await new Promise((r) => setTimeout(r, 500));

  // Assert the side effect (e.g., welcome email was "sent")
  const logs = await db.query("SELECT * FROM email_log WHERE to_email = $1", ["a@test.com"]);
  assert.strictEqual(logs.length, 1);
});
```
