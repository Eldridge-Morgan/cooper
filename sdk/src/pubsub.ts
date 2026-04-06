import { registry } from "./registry.js";
import {
  ensureConnected,
  getJetStream,
  getJetStreamManager,
  jsonCodec,
  streamName,
  consumerName,
  ensureStream,
} from "./nats.js";
import { headers as natsHeaders, AckPolicy } from "nats";

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
 *
 * Local dev uses embedded NATS with JetStream for durable delivery.
 * Falls back to in-memory if NATS is unavailable.
 */
export function topic<T = any>(name: string, config?: TopicConfig): Topic<T> {
  const subscribers = new Map<string, { handler: Function; options: any }>();
  const useDedup = config?.deliveryGuarantee === "exactly-once";
  const subject = `cooper.topic.${name}`;

  // Track whether JetStream consumers have been started
  const activeConsumers = new Set<string>();

  /**
   * Start a JetStream pull consumer for a subscriber.
   * Runs in the background, processing messages until the connection closes.
   */
  async function startConsumer(
    subName: string,
    handler: Function,
    concurrency: number
  ): Promise<void> {
    if (activeConsumers.has(subName)) return;
    activeConsumers.add(subName);

    const js = getJetStream();
    const jsm = getJetStreamManager();
    if (!js || !jsm) return;

    const stream = streamName(name);
    const durable = consumerName(subName);

    // Ensure consumer exists
    try {
      await jsm.consumers.info(stream, durable);
    } catch {
      await jsm.consumers.add(stream, {
        durable_name: durable,
        ack_policy: AckPolicy.Explicit,
        max_ack_pending: concurrency,
        filter_subject: subject,
      });
    }

    const consumer = await js.consumers.get(stream, durable);

    // Process messages in the background
    (async () => {
      try {
        const messages = await consumer.consume();
        for await (const msg of messages) {
          try {
            const data = jsonCodec.decode(msg.data);
            await handler(data);
            msg.ack();
          } catch (err) {
            console.error(
              `[cooper] Subscriber "${subName}" on topic "${name}" failed:`,
              err
            );
            // NAK with delay for retry (5 second backoff)
            msg.nak(5000);
          }
        }
      } catch (err: any) {
        // Consumer iteration ended (connection closed, etc.)
        activeConsumers.delete(subName);
        if (!err.message?.includes("closed")) {
          console.error(
            `[cooper] Consumer "${subName}" on topic "${name}" stopped:`,
            err
          );
        }
      }
    })();
  }

  const t: Topic<T> = {
    async publish(data: T) {
      const connected = await ensureConnected();

      if (connected) {
        const js = getJetStream();
        if (js) {
          await ensureStream(name, { dedup: useDedup });

          const headers =
            useDedup && data && typeof data === "object"
              ? createDedup(data as any, config?.orderBy)
              : undefined;

          await js.publish(subject, jsonCodec.encode(data), { headers });
          return;
        }
      }

      // Fallback: in-memory delivery
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

      // Start JetStream consumer in the background
      const concurrency = subConfig.concurrency ?? 1;
      ensureConnected().then(async (connected) => {
        if (connected) {
          await ensureStream(name, { dedup: useDedup });
          startConsumer(subName, subConfig.handler, concurrency).catch(
            (err) => {
              console.error(
                `[cooper] Failed to start consumer "${subName}":`,
                err
              );
            }
          );
        }
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

/**
 * Create NATS headers for exactly-once dedup.
 * Uses Nats-Msg-Id header — JetStream deduplicates within the stream's
 * duplicate_window based on this ID.
 */
function createDedup(
  data: Record<string, any>,
  orderBy?: string
): any {
  const h = natsHeaders();

  // Generate a deterministic dedup ID.
  // If an ordering key is set, use that field's value as the dedup key.
  // Otherwise, hash the entire payload for content-based dedup.
  if (orderBy && data[orderBy] !== undefined) {
    h.set("Nats-Msg-Id", `${orderBy}-${String(data[orderBy])}`);
  } else {
    const payload = JSON.stringify(data);
    let hash = 0;
    for (let i = 0; i < payload.length; i++) {
      hash = ((hash << 5) - hash + payload.charCodeAt(i)) | 0;
    }
    h.set("Nats-Msg-Id", `msg-${Math.abs(hash)}`);
  }

  return h;
}
