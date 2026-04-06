/**
 * NATS Connection Manager — singleton lazy connection to embedded NATS.
 *
 * JetStream is used for durable pub/sub with delivery guarantees.
 * Falls back gracefully if NATS is unavailable (logs warning once).
 */

import {
  connect,
  type NatsConnection,
  type JetStreamClient,
  type JetStreamManager,
  JSONCodec,
  RetentionPolicy,
  StorageType,
  AckPolicy,
} from "nats";

let nc: NatsConnection | null = null;
let js: JetStreamClient | null = null;
let jsm: JetStreamManager | null = null;
let connectPromise: Promise<boolean> | null = null;
let warnedOnce = false;

const jc = JSONCodec();

function getNatsUrl(): string {
  return process.env.COOPER_NATS_URL ?? "nats://localhost:4222";
}

async function doConnect(): Promise<boolean> {
  try {
    nc = await connect({ servers: getNatsUrl(), maxReconnectAttempts: 5 });
    js = nc.jetstream();
    jsm = await nc.jetstreamManager();
    return true;
  } catch (err: any) {
    if (!warnedOnce) {
      console.warn(
        `[cooper] NATS unavailable at ${getNatsUrl()} — pub/sub will use in-memory fallback. ${err.message}`
      );
      warnedOnce = true;
    }
    nc = null;
    js = null;
    jsm = null;
    return false;
  }
}

export async function ensureConnected(): Promise<boolean> {
  if (nc && !nc.isClosed()) return true;
  if (connectPromise) return connectPromise;
  connectPromise = doConnect().finally(() => {
    connectPromise = null;
  });
  return connectPromise;
}

export function getJetStream(): JetStreamClient | null {
  return js;
}

export function getJetStreamManager(): JetStreamManager | null {
  return jsm;
}

export function getConnection(): NatsConnection | null {
  return nc;
}

export { jc as jsonCodec };

/**
 * Sanitize a topic name into a valid NATS stream name.
 * NATS streams: alphanumeric + dash + underscore only.
 */
export function streamName(topicName: string): string {
  return "COOPER_" + topicName.replace(/[^a-zA-Z0-9_-]/g, "_").toUpperCase();
}

/**
 * Sanitize a subscriber name into a valid NATS durable consumer name.
 */
export function consumerName(subscriberName: string): string {
  return subscriberName.replace(/[^a-zA-Z0-9_-]/g, "_");
}

/**
 * Ensure a JetStream stream exists for a topic.
 * Creates it if missing, no-ops if it already exists.
 */
export async function ensureStream(
  topicName: string,
  config?: { dedup?: boolean }
): Promise<void> {
  if (!jsm) return;

  const name = streamName(topicName);
  const subject = `cooper.topic.${topicName}`;

  try {
    await jsm.streams.info(name);
  } catch {
    await jsm.streams.add({
      name,
      subjects: [subject],
      retention: RetentionPolicy.Interest,
      max_msgs: -1,
      max_bytes: -1,
      max_age: 7 * 24 * 60 * 60 * 1_000_000_000, // 7 days in nanos
      storage: StorageType.File,
      num_replicas: 1,
      duplicate_window: config?.dedup
        ? 2 * 60 * 1_000_000_000 // 2 min dedup window
        : 0,
    });
  }
}

/**
 * Ensure a JetStream stream exists for a job queue.
 * Uses WorkQueue retention — each message consumed by exactly one worker.
 */
export async function ensureQueueStream(
  queueName: string,
  config?: { dedup?: boolean }
): Promise<void> {
  if (!jsm) return;

  const name = "QUEUE_" + queueName.replace(/[^a-zA-Z0-9_-]/g, "_").toUpperCase();
  const subject = `cooper.queue.${queueName}`;

  try {
    await jsm.streams.info(name);
  } catch {
    await jsm.streams.add({
      name,
      subjects: [subject],
      retention: RetentionPolicy.Workqueue,
      max_msgs: -1,
      max_bytes: -1,
      max_age: 7 * 24 * 60 * 60 * 1_000_000_000, // 7 days in nanos
      storage: StorageType.File,
      num_replicas: 1,
      duplicate_window: config?.dedup
        ? 5 * 60 * 1_000_000_000 // 5 min dedup window for queues
        : 0,
    });
  }
}

/**
 * Ensure a JetStream stream exists for a dead-letter queue.
 * Uses Limits retention — messages stay until explicitly purged.
 */
export async function ensureDLQStream(dlqName: string): Promise<void> {
  if (!jsm) return;

  const name = "DLQ_" + dlqName.replace(/[^a-zA-Z0-9_-]/g, "_").toUpperCase();
  const subject = `cooper.dlq.${dlqName}`;

  try {
    await jsm.streams.info(name);
  } catch {
    await jsm.streams.add({
      name,
      subjects: [subject],
      retention: RetentionPolicy.Limits,
      max_msgs: 10000,
      max_bytes: -1,
      max_age: 30 * 24 * 60 * 60 * 1_000_000_000, // 30 days
      storage: StorageType.File,
      num_replicas: 1,
    });
  }
}

/**
 * Graceful shutdown — drain and close the connection.
 */
export async function closeNats(): Promise<void> {
  if (nc && !nc.isClosed()) {
    await nc.drain();
  }
}
