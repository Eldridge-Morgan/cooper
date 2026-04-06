# cooper/api

```ts
import { api } from "cooper-stack/api";
```

## `api(config, handler)`

Define an API endpoint.

### Config

| Field | Type | Default | Description |
|---|---|---|---|
| `method` | `string` | `"GET"` | HTTP method |
| `path` | `string` | required | URL path with `:param` placeholders |
| `auth` | `boolean` | `false` | Require authentication |
| `validate` | `ZodSchema` | — | Request body validation schema |
| `stream` | `"sse" \| "websocket"` | — | Enable streaming |
| `middleware` | `MiddlewareFn[]` | `[]` | Per-route middleware |

### Handler

```ts
async (input: T, principal?: P) => Promise<R>
```

- `input` — merged path params + request body (validated if schema provided)
- `principal` — auth principal (only when `auth: true`)
- Return value is serialized as JSON
