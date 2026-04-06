# Auth

Register an auth handler once. It runs automatically on every route with `auth: true`.

## Auth handler

```ts
import { authHandler } from "cooper-stack/auth";

export const auth = authHandler(async (token: string) => {
  const payload = await verifyJWT(token);
  return { userId: payload.sub, role: payload.role };
});
```

The return value becomes the **principal** — injected as the second argument into protected routes.

## Protected routes

```ts
export const adminOnly = api(
  { method: "GET", path: "/admin/stats", auth: true },
  async ({}, { userId, role }) => {
    if (role !== "admin") throw new CooperError("PERMISSION_DENIED");
    return getStats();
  }
);
```

## How it works

1. Client sends `Authorization: Bearer <token>`
2. Cooper extracts the token from the header
3. Your auth handler verifies it and returns a principal
4. The principal is passed to the route handler
5. If the token is missing or invalid, Cooper returns `401 Unauthorized`
