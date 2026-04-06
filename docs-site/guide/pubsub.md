# Pub/Sub

Typed topics for event broadcasting. Backed by NATS locally, SNS/Pub/Sub/Service Bus in production.

## Declare a topic

```ts
import { topic } from "cooper/pubsub";

export const UserSignedUp = topic<{ userId: string; email: string }>(
  "user-signed-up",
  { deliveryGuarantee: "at-least-once" }
);
```

## Publish

```ts
await UserSignedUp.publish({ userId: "u_123", email: "a@b.com" });
```

## Subscribe

```ts
export const onSignup = UserSignedUp.subscribe("send-welcome-email", {
  concurrency: 5,
  handler: async ({ userId, email }) => {
    await sendWelcomeEmail(email);
  },
});
```

## Delivery guarantees

| Option | Description |
|---|---|
| `at-least-once` | Message delivered at least once (may duplicate) |
| `exactly-once` | Message delivered exactly once (requires ordering key) |
