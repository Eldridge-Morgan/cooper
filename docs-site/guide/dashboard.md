# Dev Dashboard

Cooper includes a live development dashboard at `http://localhost:9500` when running `cooper run`.

## Tabs

### Map

Visual service map showing your application architecture:
- **Service nodes** — extracted from `services/` directory structure
- **Topic nodes** — pub/sub topics with animated flow arrows
- **Queue nodes** — job queues with service connections
- **Database indicators** — shown within service nodes
- **Cron indicators** — shown within service nodes

Uses Dagre.js for automatic graph layout.

### Routes

Table of all registered API routes:

| Column | Description |
|---|---|
| Method | HTTP method (GET, POST, etc.) |
| Path | URL pattern with parameters |
| Handler | Export name |
| Source | Source file path |
| Auth | Shows `AUTH` badge if `auth: true` |

### Explorer

Interactive API testing:
1. Select a route from the dropdown
2. Add headers (e.g., `Authorization: Bearer ...`)
3. Add request body (JSON)
4. Click Send
5. View response with status code, body, and timing

Path parameters (`:id`) are prompted when sending.

### Log

Real-time request log updated via Server-Sent Events:
- Timestamp
- HTTP method
- Path
- Status code (color-coded: green < 300, yellow >= 300)
- Response duration in milliseconds

Auto-populates as you hit API endpoints.

### Events

Live feed of all infrastructure events:
- **Request events** — every API call with method, path, status, duration
- **Pub/sub events** — topic messages published and consumed
- **Queue events** — jobs enqueued, processed, failed
- **Cron events** — scheduled job executions

Filter by event type using the buttons: All | Pub/Sub | Queues.

Color-coded badges:
- Green: pub/sub
- Blue: queue
- Orange: cron
- Purple: request

### Crons

Execution history for cron jobs:

| Column | Description |
|---|---|
| Name | Cron job name |
| Status | Success or error |
| Duration | Execution time in ms |
| Time | When it ran |

## Architecture

The dashboard uses Server-Sent Events (SSE) for real-time updates:

```
API Handler → events_tx.send() → broadcast channel → /_cooper/events (SSE) → Dashboard JS
```

The SSE connection auto-reconnects with a 3-second backoff if the server restarts.

## Port

The dashboard defaults to port 9500. If taken, it tries 9501-9509. The actual port is printed on startup:

```
📊 Dashboard at http://localhost:9500
```

## Design

Minimal monochrome UI with monospace typography (SF Mono, JetBrains Mono, Fira Code). Animated ASCII logo on load.
