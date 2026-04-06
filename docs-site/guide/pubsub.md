# Pub/Sub

Typed topics for event broadcasting. Backed by NATS JetStream locally with durable delivery. Maps to SNS/Pub/Sub/Service Bus in production.

## Declare a topic

```ts
import { topic } from "cooper-stack/pubsub";

export const UserSignedUp = topic<{ userId: string; email: string }>(
  "user-signed-up",
  { deliveryGuarantee: "at-least-once" }
);
```

## Publish

```ts
await UserSignedUp.publish({ userId: "u_123", email: "a@b.com" });
```

Messages are published to NATS JetStream and persisted to disk. They survive process restarts.

## Subscribe

```ts
export const onSignup = UserSignedUp.subscribe("send-welcome-email", {
  concurrency: 5,
  handler: async ({ userId, email }) => {
    await sendWelcomeEmail(email);
  },
});
```

Each subscriber gets a durable JetStream pull consumer. Messages are explicitly acked after the handler succeeds. If the handler throws, the message is NAK'd with a 5-second backoff and redelivered.

## Delivery guarantees

| Option | Behavior | Implementation |
|---|---|---|
| `at-least-once` | Message delivered at least once (may duplicate) | Durable JetStream consumer with explicit ack |
| `exactly-once` | Duplicate messages rejected within dedup window | Deterministic `Nats-Msg-Id` header, 2-minute JetStream dedup window |

### Exactly-once with ordering key

When `orderBy` is set, the field's value becomes the dedup key. Publishing the same value twice within the dedup window is rejected:

```ts
const PaymentProcessed = topic<{ paymentId: string; amount: number }>(
  "payment-processed",
  {
    deliveryGuarantee: "exactly-once",
    orderBy: "paymentId",
  }
);

// First publish succeeds
await PaymentProcessed.publish({ paymentId: "pay_123", amount: 50 });

// Duplicate rejected (same paymentId within 2 minutes)
await PaymentProcessed.publish({ paymentId: "pay_123", amount: 50 });

// Different paymentId succeeds
await PaymentProcessed.publish({ paymentId: "pay_456", amount: 100 });
```

Without `orderBy`, dedup is based on the full payload hash.

## Multiple subscribers

All subscribers receive every message (fan-out). Each subscriber maintains its own consumer position:

```ts
const OrderPlaced = topic<{ orderId: string; total: number }>("order-placed");

OrderPlaced.subscribe("update-inventory", {
  handler: async ({ orderId }) => {
    await decrementStock(orderId);
  },
});

OrderPlaced.subscribe("send-confirmation", {
  handler: async ({ orderId }) => {
    await sendOrderEmail(orderId);
  },
});

OrderPlaced.subscribe("notify-warehouse", {
  concurrency: 10,
  handler: async ({ orderId }) => {
    await notifyWarehouse(orderId);
  },
});

// All three subscribers receive every published message
await OrderPlaced.publish({ orderId: "ord_789", total: 149.99 });
```

## Concurrency

The `concurrency` option controls how many messages a subscriber processes in parallel:

```ts
UserSignedUp.subscribe("heavy-processing", {
  concurrency: 20, // process up to 20 messages simultaneously
  handler: async (data) => {
    await expensiveOperation(data);
  },
});
```

This maps to JetStream's `max_ack_pending` — at most N unacknowledged messages in flight.

## Persistence

Messages are stored on disk via NATS JetStream with a 7-day retention. This means:

- Messages survive `cooper run` restarts
- Subscribers that start after a publish will receive queued messages
- Unacknowledged messages are redelivered

## Fallback

If NATS is unavailable (e.g., `nats-server` binary not found), pub/sub falls back to in-memory direct delivery. A warning is logged once:

```
[cooper] NATS unavailable at nats://localhost:4222 — pub/sub will use in-memory fallback.
```

In-memory fallback works for development but messages are lost on restart.

## Combining with routes

A common pattern is to publish events from API handlers:

```ts
import { api } from "cooper-stack/api";
import { topic } from "cooper-stack/pubsub";
import { db } from "./db";

const UserCreated = topic<{ userId: string; email: string }>("user-created");

export const createUser = api(
  { method: "POST", path: "/users" },
  async (input: { name: string; email: string }) => {
    const user = await db.insert("users", input);
    await UserCreated.publish({ userId: user.id, email: user.email });
    return user;
  }
);
```

## Cloud mapping

| Local | AWS | GCP | Azure |
|---|---|---|---|
| NATS JetStream | SNS + SQS | Cloud Pub/Sub | Service Bus |

## Config reference

| Option | Type | Default | Description |
|---|---|---|---|
| `deliveryGuarantee` | `"at-least-once" \| "exactly-once"` | `"at-least-once"` | Delivery semantics |
| `orderBy` | `string` | — | Field name used as dedup key for exactly-once |

### Subscribe options

| Option | Type | Default | Description |
|---|---|---|---|
| `concurrency` | `number` | `1` | Max parallel message processing |
| `handler` | `(data: T) => Promise<void>` | required | Message handler function |
