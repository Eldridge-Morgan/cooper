import { registry } from "./registry.js";

export interface TopicConfig {
  deliveryGuarantee?: "at-least-once" | "exactly-once";
  orderBy?: string;
}

export interface SubscribeConfig {
  concurrency?: number;
  handler: (data: any) => Promise<void>;
}

export interface Topic<T> {
  publish(data: T): Promise<void>;
  subscribe(name: string, config: SubscribeConfig): any;
}

/**
 * Declare a typed Pub/Sub topic.
 *
 * ```ts
 * export const UserSignedUp = topic<{ userId: string; email: string }>(
 *   "user-signed-up",
 *   { deliveryGuarantee: "at-least-once" }
 * );
 * ```
 */
export function topic<T = any>(name: string, config?: TopicConfig): Topic<T> {
  const subscribers = new Map<string, { handler: Function; options: any }>();

  const t: Topic<T> = {
    async publish(data: T) {
      // In local dev, deliver directly to subscribers
      // In production, publish to NATS/SNS/Pub/Sub
      const natsUrl = process.env.COOPER_NATS_URL ?? "nats://localhost:4222";

      // Direct local delivery for development
      for (const [subName, sub] of subscribers) {
        try {
          await sub.handler(data);
        } catch (err) {
          console.error(`[cooper] Subscriber "${subName}" failed:`, err);
        }
      }
    },

    subscribe(subName: string, subConfig: SubscribeConfig) {
      subscribers.set(subName, {
        handler: subConfig.handler,
        options: subConfig,
      });

      registry.registerTopic(name, {
        name,
        subscribers,
      });

      return {
        _cooper_type: "subscription",
        topic: name,
        name: subName,
      };
    },
  };

  registry.registerTopic(name, { name, subscribers });

  return t;
}
