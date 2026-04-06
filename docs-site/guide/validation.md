# Validation

Cooper uses [Zod](https://zod.dev) for request validation. Invalid requests never reach your handler.

## Setup

```ts
import { api } from "cooper/api";
import { z } from "zod";

const CreateUserSchema = z.object({
  name: z.string().min(1).max(100),
  email: z.string().email(),
  age: z.number().int().min(0).max(150),
});

export const createUser = api(
  { method: "POST", path: "/users", validate: CreateUserSchema },
  async (req) => {
    // req is fully typed: { name: string; email: string; age: number }
    // invalid requests get a 422 before reaching here
  }
);
```

## Error response

When validation fails, the client receives:

```json
{
  "error": {
    "code": "VALIDATION_FAILED",
    "message": "name: String must contain at least 1 character(s); email: Invalid email"
  }
}
```

HTTP status: `422 Unprocessable Entity`.

## Partial updates

```ts
const UpdateUserSchema = z.object({
  name: z.string().min(1).max(100).optional(),
  email: z.string().email().optional(),
});

export const updateUser = api(
  { method: "PATCH", path: "/users/:id", validate: UpdateUserSchema },
  async ({ id, ...updates }) => {
    // only provided fields are present
  }
);
```
