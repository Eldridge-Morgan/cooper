/**
 * Internal registry — Cooper's Rust runtime communicates with JS workers
 * by referencing registered handlers by name. This module tracks all
 * declarations (routes, topics, crons, queues, etc.) so the bridge
 * can look them up at runtime.
 */

export interface RegisteredRoute {
  method: string;
  path: string;
  auth: boolean;
  stream?: "sse" | "websocket";
  validate?: any; // Zod schema
  middleware: MiddlewareFn[];
  handler: Function;
  exportName: string;
  sourceFile: string;
}

export interface RegisteredTopic {
  name: string;
  subscribers: Map<string, { handler: Function; options: any }>;
}

export interface RegisteredCron {
  name: string;
  schedule: string;
  handler: Function;
}

export interface RegisteredQueue {
  name: string;
  options: any;
  worker?: { name: string; handler: Function; onFailure?: Function };
}

export interface RegisteredDatabase {
  name: string;
  engine: string;
  pool: any;
}

export interface RegisteredCache {
  name: string;
  options: any;
}

export interface RegisteredBucket {
  name: string;
  options: any;
}

export type MiddlewareFn = (req: any, next: (req: any) => Promise<any>) => Promise<any>;

export type AuthHandlerFn = (token: string) => Promise<Record<string, any>>;

class Registry {
  routes: Map<string, RegisteredRoute> = new Map();
  topics: Map<string, RegisteredTopic> = new Map();
  crons: Map<string, RegisteredCron> = new Map();
  queues: Map<string, RegisteredQueue> = new Map();
  databases: Map<string, RegisteredDatabase> = new Map();
  caches: Map<string, RegisteredCache> = new Map();
  buckets: Map<string, RegisteredBucket> = new Map();
  secrets: Map<string, () => Promise<string>> = new Map();
  globalMiddleware: MiddlewareFn[] = [];
  authHandler: AuthHandlerFn | null = null;

  registerRoute(key: string, route: RegisteredRoute) {
    this.routes.set(key, route);
  }

  registerTopic(name: string, topic: RegisteredTopic) {
    this.topics.set(name, topic);
  }

  registerCron(name: string, cron: RegisteredCron) {
    this.crons.set(name, cron);
  }

  registerQueue(name: string, queue: RegisteredQueue) {
    this.queues.set(name, queue);
  }

  registerDatabase(name: string, db: RegisteredDatabase) {
    this.databases.set(name, db);
  }

  setAuthHandler(handler: AuthHandlerFn) {
    this.authHandler = handler;
  }

  addGlobalMiddleware(mw: MiddlewareFn) {
    this.globalMiddleware.push(mw);
  }
}

export const registry = new Registry();
