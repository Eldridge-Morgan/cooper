# Queues

Job queues for work distribution — distinct from pub/sub. One producer, one consumer pool, with retries and dead-letter handling.

## Declare

```ts
import { queue } from "cooper/queue";

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
  delay: "5m",        // defer execution
  priority: "high",   // jump the queue
  dedupeKey: userId,  // skip if same key already enqueued
});
```

## Worker

```ts
export const emailWorker = EmailQueue.worker("send-emails", {
  handler: async ({ to, subject, body }) => {
    await sendgrid.send({ to, subject, body });
  },
  onFailure: async (job, error) => {
    console.error(`Failed: ${error.message}`);
  },
});
```

## Dead-letter queue

```ts
export const EmailDLQ = queue("email-dlq");

// Replay failed jobs
const jobs = await EmailDLQ.list();
for (const job of jobs) {
  await EmailQueue.enqueue(job.data);
  await EmailDLQ.delete(job.id);
}
```

## Cloud mapping

| Local | AWS | GCP | Azure | Fly |
|---|---|---|---|---|
| NATS JetStream | SQS | Cloud Tasks | Storage Queues | Upstash QStash |
