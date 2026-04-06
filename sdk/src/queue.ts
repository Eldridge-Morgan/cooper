import { registry } from "./registry.js";
import {
  ensureConnected,
  getJetStream,
  getJetStreamManager,
  jsonCodec,
  consumerName,
  ensureQueueStream,
  ensureDLQStream,
} from "./nats.js";
import { headers as natsHeaders, AckPolicy } from "nats";

export interface QueueConfig {
  concurrency?: number;
  retries?: number;
  retryDelay?: "fixed" | "exponential";
  timeout?: string;
  deadLetter?: string;
}

export interface EnqueueOptions {
  delay?: string;
  priority?: "low" | "normal" | "high";
  dedupeKey?: string;
}

export interface QueueClient<T> {
  enqueue(data: T, opts?: EnqueueOptions): Promise<void>;
  worker(
    name: string,
    config: {
      handler: (data: T) => Promise<void>;
      onFailure?: (data: T, error: Error) => Promise<void>;
    }
  ): any;
  list(): Promise<{ id: string; data: T }[]>;
  delete(id: string): Promise<void>;
}

const parseDuration = (dur: string): number => {
  const m = dur.match(/^(\d+)(s|m|h|d)$/);
  if (!m) return 0;
  const mult: Record<string, number> = {
    s: 1000,
    m: 60000,
    h: 3600000,
    d: 86400000,
  };
  return parseInt(m[1]) * (mult[m[2]] ?? 1000);
};

/**
 * Declare a job queue.
 *
 * ```ts
 * export const EmailQueue = queue<{ to: string; subject: string; body: string }>(
 *   "email-queue",
 *   { concurrency: 10, retries: 3, retryDelay: "exponential", timeout: "30s", deadLetter: "email-dlq" }
 * );
 * ```
 *
 * Local dev uses NATS JetStream for persistent, durable queues.
 * Falls back to in-memory if NATS is unavailable.
 */
export function queue<T = any>(
  name: string,
  config?: QueueConfig
): QueueClient<T> {
  const subject = `cooper.queue.${name}`;
  const concurrency = config?.concurrency ?? 1;
  const maxRetries = config?.retries ?? 0;
  const timeoutMs = config?.timeout ? parseDuration(config.timeout) : 0;
  const dlqName = config?.deadLetter;

  // In-memory fallback state
  const memJobs: {
    id: string;
    data: T;
    attempts: number;
    priority: string;
    scheduledAt: number;
  }[] = [];
  let memProcessing = false;
  let workerHandler: ((data: T) => Promise<void>) | null = null;
  let failureHandler: ((data: T, error: Error) => Promise<void>) | null = null;
  let consumerStarted = false;

  // Track retry attempts per message (NATS redelivers original message on NAK)
  const attemptTracker = new Map<number, number>();

  // In-memory fallback processor
  const processMemQueue = async () => {
    if (memProcessing || !workerHandler) return;
    memProcessing = true;

    while (memJobs.length > 0) {
      const batch = memJobs.splice(0, concurrency);
      await Promise.allSettled(
        batch.map(async (job) => {
          if (job.scheduledAt > Date.now()) {
            memJobs.push(job);
            return;
          }
          try {
            await executeWithTimeout(workerHandler!, job.data);
          } catch (err) {
            job.attempts++;
            if (job.attempts <= maxRetries) {
              const delay =
                config?.retryDelay === "exponential"
                  ? Math.pow(2, job.attempts - 1) * 1000
                  : 1000;
              job.scheduledAt = Date.now() + delay;
              memJobs.push(job);
            } else if (failureHandler) {
              await failureHandler(job.data, err as Error);
            }
          }
        })
      );
    }

    memProcessing = false;
  };

  async function executeWithTimeout(
    handler: (data: T) => Promise<void>,
    data: T
  ): Promise<void> {
    if (!timeoutMs) {
      return handler(data);
    }
    const result = await Promise.race([
      handler(data),
      new Promise<never>((_, reject) =>
        setTimeout(
          () => reject(new Error(`Job timed out after ${config!.timeout}`)),
          timeoutMs
        )
      ),
    ]);
    return result;
  }

  /**
   * Start a JetStream consumer to process queue jobs.
   */
  async function startConsumer(
    workerName: string,
    handler: (data: T) => Promise<void>,
    onFailure?: (data: T, error: Error) => Promise<void>
  ): Promise<void> {
    if (consumerStarted) return;
    consumerStarted = true;

    const js = getJetStream();
    const jsm = getJetStreamManager();
    if (!js || !jsm) return;

    const streamName =
      "QUEUE_" + name.replace(/[^a-zA-Z0-9_-]/g, "_").toUpperCase();
    const durable = consumerName(workerName);

    // Ensure consumer exists
    try {
      await jsm.consumers.info(streamName, durable);
    } catch {
      await jsm.consumers.add(streamName, {
        durable_name: durable,
        ack_policy: AckPolicy.Explicit,
        max_ack_pending: concurrency,
        filter_subject: subject,
        // Redeliver unacked messages after ack_wait
        ack_wait: 30 * 1_000_000_000, // 30 seconds in nanos
      });
    }

    const consumer = await js.consumers.get(streamName, durable);

    // Process jobs in the background
    (async () => {
      try {
        const messages = await consumer.consume();
        for await (const msg of messages) {
          let jobData: T;
          try {
            jobData = jsonCodec.decode(msg.data) as T;
          } catch {
            // Malformed message — ack to remove from queue
            msg.ack();
            continue;
          }

          // Track retries using sequence number — msg.seq is the stream seq
          const seq = msg.seq;
          const attempts = attemptTracker.get(seq) ?? 0;

          try {
            await executeWithTimeout(handler, jobData);
            msg.ack();
            attemptTracker.delete(seq);
          } catch (err) {
            const nextAttempt = attempts + 1;
            attemptTracker.set(seq, nextAttempt);

            if (nextAttempt <= maxRetries) {
              // NAK with backoff delay for retry
              const delay =
                config?.retryDelay === "exponential"
                  ? Math.pow(2, nextAttempt - 1) * 1000
                  : 1000;
              msg.nak(delay);
            } else {
              // Max retries exceeded — move to DLQ if configured
              if (dlqName) {
                try {
                  await moveToDLQ(dlqName, name, jobData, err as Error);
                } catch (dlqErr) {
                  console.error(
                    `[cooper] Failed to move job to DLQ "${dlqName}":`,
                    dlqErr
                  );
                }
              }

              if (onFailure) {
                try {
                  await onFailure(jobData, err as Error);
                } catch (failErr) {
                  console.error(
                    `[cooper] onFailure handler for queue "${name}" threw:`,
                    failErr
                  );
                }
              }

              // Ack to remove from main queue (it's in DLQ now)
              msg.ack();
              attemptTracker.delete(seq);
            }
          }
        }
      } catch (err: any) {
        consumerStarted = false;
        if (!err.message?.includes("closed")) {
          console.error(
            `[cooper] Queue consumer "${name}" stopped:`,
            err
          );
        }
      }
    })();
  }

  const client: QueueClient<T> = {
    async enqueue(data: T, opts?: EnqueueOptions) {
      const connected = await ensureConnected();

      if (connected) {
        const js = getJetStream();
        if (js) {
          await ensureQueueStream(name, { dedup: !!opts?.dedupeKey });

          const h = natsHeaders();

          // Dedup key
          if (opts?.dedupeKey) {
            h.set("Nats-Msg-Id", opts.dedupeKey);
          }

          // Priority as header (for visibility — JetStream doesn't natively prioritize)
          if (opts?.priority && opts.priority !== "normal") {
            h.set("Cooper-Priority", opts.priority);
          }

          // Initial attempt count
          h.set("Cooper-Attempts", "0");

          if (opts?.delay) {
            // For delayed jobs: JetStream doesn't have native delay.
            // We publish immediately but set a header; consumer-side can
            // NAK with delay or we use a timer.
            const delayMs = parseDuration(opts.delay);
            h.set("Cooper-Delay-Until", String(Date.now() + delayMs));
          }

          await js.publish(subject, jsonCodec.encode(data), { headers: h });
          return;
        }
      }

      // Fallback: in-memory
      const delay = opts?.delay ? parseDuration(opts.delay) : 0;
      memJobs.push({
        id: crypto.randomUUID(),
        data,
        attempts: 0,
        priority: opts?.priority ?? "normal",
        scheduledAt: Date.now() + delay,
      });

      const priorityOrder: Record<string, number> = {
        high: 0,
        normal: 1,
        low: 2,
      };
      memJobs.sort(
        (a, b) =>
          (priorityOrder[a.priority] ?? 1) - (priorityOrder[b.priority] ?? 1)
      );

      setImmediate(() => processMemQueue());
    },

    worker(workerName: string, workerConfig) {
      workerHandler = workerConfig.handler;
      failureHandler = workerConfig.onFailure ?? null;

      registry.registerQueue(name, {
        name,
        options: config ?? {},
        worker: {
          name: workerName,
          handler: workerConfig.handler,
          onFailure: workerConfig.onFailure,
        },
      });

      // Start JetStream consumer
      ensureConnected().then(async (connected) => {
        if (connected) {
          await ensureQueueStream(name);
          if (dlqName) await ensureDLQStream(dlqName);
          startConsumer(
            workerName,
            workerConfig.handler,
            workerConfig.onFailure
          ).catch((err) => {
            console.error(
              `[cooper] Failed to start queue consumer "${name}":`,
              err
            );
          });
        }
      });

      // Kick in-memory fallback processing
      setImmediate(() => processMemQueue());

      return { _cooper_type: "queue_worker", queue: name, name: workerName };
    },

    async list() {
      const connected = await ensureConnected();

      if (connected) {
        const jsm = getJetStreamManager();
        if (jsm) {
          const streamName =
            "QUEUE_" + name.replace(/[^a-zA-Z0-9_-]/g, "_").toUpperCase();
          try {
            const info = await jsm.streams.info(streamName);
            // Return stream message count as approximation
            // JetStream doesn't support listing individual messages directly
            return Array.from(
              { length: Number(info.state.messages) },
              (_, i) => ({
                id: `js-${i}`,
                data: {} as T,
              })
            );
          } catch {
            return [];
          }
        }
      }

      return memJobs.map((j) => ({ id: j.id, data: j.data }));
    },

    async delete(id: string) {
      // In-memory fallback
      const idx = memJobs.findIndex((j) => j.id === id);
      if (idx >= 0) memJobs.splice(idx, 1);
    },
  };

  registry.registerQueue(name, { name, options: config ?? {} });

  return client;
}

/**
 * Move a failed job to the dead-letter queue.
 */
async function moveToDLQ<T>(
  dlqName: string,
  sourceQueue: string,
  data: T,
  error: Error
): Promise<void> {
  const js = getJetStream();
  if (!js) return;

  await ensureDLQStream(dlqName);

  const dlqSubject = `cooper.dlq.${dlqName}`;
  const h = natsHeaders();
  h.set("Cooper-Source-Queue", sourceQueue);
  h.set("Cooper-Error", error.message.slice(0, 256));
  h.set("Cooper-Failed-At", new Date().toISOString());

  await js.publish(dlqSubject, jsonCodec.encode(data), { headers: h });
}
