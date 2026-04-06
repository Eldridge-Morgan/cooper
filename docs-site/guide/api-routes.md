# API Routes

Define endpoints with the `api()` function. Cooper's Rust runtime handles HTTP serving — your handler just returns data.

## Basic route

```ts
import { api } from "cooper/api";

export const getUser = api(
  { method: "GET", path: "/users/:id" },
  async ({ id }: { id: string }) => {
    const user = await db.queryRow("SELECT * FROM users WHERE id = $1", [id]);
    return { user };
  }
);
```

## Route config

| Option | Type | Description |
|---|---|---|
| `method` | `GET \| POST \| PUT \| PATCH \| DELETE` | HTTP method (default: `GET`) |
| `path` | `string` | URL path with `:param` placeholders |
| `auth` | `boolean` | Require authentication (default: `false`) |
| `validate` | Zod schema | Request body validation |
| `stream` | `"sse" \| "websocket"` | Streaming response type |
| `middleware` | `MiddlewareFn[]` | Per-route middleware |

## Path parameters

```ts
// Path: /users/:userId/posts/:postId
export const getPost = api(
  { method: "GET", path: "/users/:userId/posts/:postId" },
  async ({ userId, postId }: { userId: string; postId: string }) => {
    // params are extracted and typed
  }
);
```

## Request body

For `POST`, `PUT`, `PATCH` — the request body is parsed as JSON and passed as the first argument:

```ts
export const createUser = api(
  { method: "POST", path: "/users" },
  async (body: { name: string; email: string }) => {
    // body is the parsed JSON request body
  }
);
```

## Protected routes

```ts
export const deleteUser = api(
  { method: "DELETE", path: "/users/:id", auth: true },
  async ({ id }, principal) => {
    // principal is injected by your auth handler
    if (principal.role !== "admin") {
      throw new CooperError("PERMISSION_DENIED", "Admins only");
    }
  }
);
```

See [Auth](/guide/auth) for setting up the auth handler.

## Per-route middleware

```ts
import { middleware } from "cooper/middleware";

const rateLimiter = middleware(async (req, next) => {
  // ...
  return next(req);
});

export const sensitiveRoute = api(
  { method: "POST", path: "/transfer", middleware: [rateLimiter] },
  async (body) => { /* ... */ }
);
```
