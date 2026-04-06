# Errors

Structured errors with automatic HTTP status mapping.

## Throwing errors

```ts
import { CooperError } from "cooper";

throw new CooperError("NOT_FOUND", "User not found");
throw new CooperError("UNAUTHORIZED", "Token expired");
throw new CooperError("PERMISSION_DENIED", "Admins only");
throw new CooperError("RATE_LIMITED", "Slow down");
throw new CooperError("INVALID_ARGUMENT", "Email is malformed");
throw new CooperError("INTERNAL", "Unexpected failure");
```

## Error codes → HTTP status

| Code | HTTP Status |
|---|---|
| `NOT_FOUND` | 404 |
| `UNAUTHORIZED` | 401 |
| `PERMISSION_DENIED` | 403 |
| `RATE_LIMITED` | 429 |
| `INVALID_ARGUMENT` | 400 |
| `VALIDATION_FAILED` | 422 |
| `INTERNAL` | 500 |

## Client response

Every error returns the same shape:

```json
{
  "error": {
    "code": "NOT_FOUND",
    "message": "User not found"
  }
}
```

## Rate limiting and Retry-After

When a `RATE_LIMITED` error is thrown, Cooper automatically includes a `Retry-After` header in the HTTP response. You can specify the delay (in seconds) using the `retryAfter` field:

```ts
throw new CooperError("RATE_LIMITED", "Slow down", { retryAfter: 30 });
```

This produces:

```
HTTP/1.1 429 Too Many Requests
Retry-After: 30
```

```json
{
  "error": {
    "code": "RATE_LIMITED",
    "message": "Slow down",
    "retryAfter": 30
  }
}
```

If `retryAfter` is omitted, Cooper defaults to `60` seconds.
