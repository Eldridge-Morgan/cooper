# Queues

Job queues for background work with retries, timeouts, deduplication, and dead-letter handling. Backed by NATS JetStream locally.

## Declare

```ts
import { queue } from "cooper-stack/queue";

export const EmailQueue = queue<{ to: string; subject: string; body: string }>(
  "email-queue",
  {
    concurrency: 10,
    retries: 3,
    retryDelay: "exponential",  // 1s, 2s, 4s...
    timeout: "30s",
    deadLetter: "email-dlq",
  }
);
```

## Enqueue

```ts
await EmailQueue.enqueue({ to: "user@example.com", subject: "Welcome!", body: "..." });

// With options
await EmailQueue.enqueue(data, {
  delay: "5m",              // defer execution by 5 minutes
  priority: "high",         // priority hint (header-based)
  dedupeKey: "user_123",    // reject duplicate within 5-minute window
});
```

### Deduplication

The `dedupeKey` maps to a NATS `Nats-Msg-Id` header. JetStream rejects messages with the same ID within a 5-minute window:

```ts
// First enqueue succeeds
await EmailQueue.enqueue(data, { dedupeKey: "welcome-user-123" });

// Duplicate rejected silently
await EmailQueue.enqueue(data, { dedupeKey: "welcome-user-123" });
```

## Worker

```ts
export const emailWorker = EmailQueue.worker("send-emails", {
  handler: async ({ to, subject, body }) => {
    await sendgrid.send({ to, subject, body });
  },
  onFailure: async (data, error) => {
    console.error(`Email to ${data.to} failed permanently: ${error.message}`);
    // Alert, log to error tracker, etc.
  },
});
```

Workers are durable JetStream pull consumers. Each message is explicitly acked after the handler succeeds.

## Retry behavior

When a handler throws, the job is retried according to the `retryDelay` strategy:

| Strategy | Delays | Formula |
|---|---|---|
| `"exponential"` | 1s, 2s, 4s, 8s, 16s... | `2^(attempt-1) * 1000ms` |
| `"fixed"` | 1s, 1s, 1s, 1s... | `1000ms` |

After `retries` attempts are exhausted:
1. The job is moved to the dead-letter queue (if `deadLetter` is configured)
2. `onFailure` is called with the job data and final error
3. The job is acked (removed from the main queue)

## Timeout

The `timeout` option kills jobs that take too long:

```ts
const VideoQueue = queue<{ url: string }>("video-encode", {
  timeout: "2m",    // fail after 2 minutes
  retries: 1,
  deadLetter: "video-dlq",
});
```

If a handler exceeds the timeout, it's treated as a failure and follows the retry/DLQ flow.

Supported duration formats: `"30s"`, `"5m"`, `"2h"`, `"1d"`.

## Dead-letter queue

Failed jobs (after max retries) are published to a separate JetStream stream with metadata headers:

```ts
const EmailDLQ = queue("email-dlq");

// List failed jobs
const jobs = await EmailDLQ.list();

// Replay a failed job
for (const job of jobs) {
  await EmailQueue.enqueue(job.data);
  await EmailDLQ.delete(job.id);
}
```

Each DLQ message includes headers:
- `Cooper-Source-Queue` — the queue that produced the failure
- `Cooper-Error` — the error message (truncated to 256 chars)
- `Cooper-Failed-At` — ISO timestamp of the failure

DLQ streams have a 30-day retention and 10,000 message cap.

## Concurrency

The `concurrency` option controls parallel job processing:

```ts
const ImportQueue = queue<{ fileUrl: string }>("csv-import", {
  concurrency: 5, // process 5 files simultaneously
});
```

This maps to JetStream's `max_ack_pending`. The consumer won't receive more than N unacknowledged messages.

## Persistence

Jobs are stored on disk via NATS JetStream with a 7-day retention. Jobs survive process restarts. Workers that start after jobs are enqueued will begin processing immediately.

## Combining with routes

Offload slow work from API handlers:

```ts
import { api } from "cooper-stack/api";
import { queue } from "cooper-stack/queue";

const ReportQueue = queue<{ userId: string; format: string }>("reports", {
  timeout: "5m",
  retries: 2,
});

export const generateReport = api(
  { method: "POST", path: "/reports" },
  async (input: { userId: string; format: string }) => {
    await ReportQueue.enqueue(input);
    return { status: "queued" };
  }
);

ReportQueue.worker("report-builder", {
  handler: async ({ userId, format }) => {
    const data = await fetchUserData(userId);
    const report = await buildReport(data, format);
    await uploadReport(userId, report);
  },
});
```

## Fallback

If NATS is unavailable, queues fall back to in-memory processing with the same retry logic. Jobs are lost on restart in fallback mode.

## Config reference

### Queue options

| Option | Type | Default | Description |
|---|---|---|---|
| `concurrency` | `number` | `1` | Max parallel job processing |
| `retries` | `number` | `0` | Max retry attempts after failure |
| `retryDelay` | `"fixed" \| "exponential"` | `"fixed"` | Retry backoff strategy |
| `timeout` | `string` | — | Max job duration (`"30s"`, `"5m"`, etc.) |
| `deadLetter` | `string` | — | Name of the dead-letter queue |

### Enqueue options

| Option | Type | Default | Description |
|---|---|---|---|
| `delay` | `string` | — | Defer execution (`"10s"`, `"5m"`, etc.) |
| `priority` | `"low" \| "normal" \| "high"` | `"normal"` | Priority hint |
| `dedupeKey` | `string` | — | Deduplication key (5-minute window) |

## Cloud mapping

| Local | AWS | GCP | Azure | Fly |
|---|---|---|---|---|
| NATS JetStream | SQS | Cloud Tasks | Storage Queues | Upstash QStash |
