import { registry } from "./registry.js";

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
  worker(name: string, config: { handler: (data: T) => Promise<void>; onFailure?: (data: T, error: Error) => Promise<void> }): any;
  list(): Promise<{ id: string; data: T }[]>;
  delete(id: string): Promise<void>;
}

/**
 * Declare a job queue.
 *
 * ```ts
 * export const EmailQueue = queue<{ to: string; subject: string; body: string }>(
 *   "email-queue",
 *   { concurrency: 10, retries: 3, retryDelay: "exponential", timeout: "30s", deadLetter: "email-dlq" }
 * );
 * ```
 */
export function queue<T = any>(name: string, config?: QueueConfig): QueueClient<T> {
  // In-memory queue for local dev — backed by NATS JetStream
  // In production: SQS, Cloud Tasks, Azure Storage Queues, or QStash
  const jobs: { id: string; data: T; attempts: number; priority: string; scheduledAt: number }[] = [];
  let processing = false;
  let workerHandler: ((data: T) => Promise<void>) | null = null;
  let failureHandler: ((data: T, error: Error) => Promise<void>) | null = null;
  const concurrency = config?.concurrency ?? 1;
  const maxRetries = config?.retries ?? 0;

  const parseDuration = (dur: string): number => {
    const m = dur.match(/^(\d+)(s|m|h|d)$/);
    if (!m) return 0;
    const mult: Record<string, number> = { s: 1000, m: 60000, h: 3600000, d: 86400000 };
    return parseInt(m[1]) * (mult[m[2]] ?? 1000);
  };

  const processQueue = async () => {
    if (processing || !workerHandler) return;
    processing = true;

    while (jobs.length > 0) {
      const batch = jobs.splice(0, concurrency);
      await Promise.allSettled(
        batch.map(async (job) => {
          if (job.scheduledAt > Date.now()) {
            jobs.push(job); // not ready yet
            return;
          }
          try {
            await workerHandler!(job.data);
          } catch (err) {
            job.attempts++;
            if (job.attempts <= maxRetries) {
              const delay = config?.retryDelay === "exponential"
                ? Math.pow(2, job.attempts - 1) * 1000
                : 1000;
              job.scheduledAt = Date.now() + delay;
              jobs.push(job);
            } else if (failureHandler) {
              await failureHandler(job.data, err as Error);
            }
          }
        })
      );
    }

    processing = false;
  };

  const client: QueueClient<T> = {
    async enqueue(data: T, opts?: EnqueueOptions) {
      const id = crypto.randomUUID();
      const delay = opts?.delay ? parseDuration(opts.delay) : 0;
      jobs.push({
        id,
        data,
        attempts: 0,
        priority: opts?.priority ?? "normal",
        scheduledAt: Date.now() + delay,
      });

      // Sort by priority
      const priorityOrder: Record<string, number> = { high: 0, normal: 1, low: 2 };
      jobs.sort((a, b) => (priorityOrder[a.priority] ?? 1) - (priorityOrder[b.priority] ?? 1));

      // Trigger processing
      setImmediate(() => processQueue());
    },

    worker(workerName: string, workerConfig) {
      workerHandler = workerConfig.handler;
      failureHandler = workerConfig.onFailure ?? null;

      registry.registerQueue(name, {
        name,
        options: config ?? {},
        worker: { name: workerName, handler: workerConfig.handler, onFailure: workerConfig.onFailure },
      });

      // Start processing any already-queued jobs
      setImmediate(() => processQueue());

      return { _cooper_type: "queue_worker", queue: name, name: workerName };
    },

    async list() {
      return jobs.map((j) => ({ id: j.id, data: j.data }));
    },

    async delete(id: string) {
      const idx = jobs.findIndex((j) => j.id === id);
      if (idx >= 0) jobs.splice(idx, 1);
    },
  };

  registry.registerQueue(name, { name, options: config ?? {} });

  return client;
}
