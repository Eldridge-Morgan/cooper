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
