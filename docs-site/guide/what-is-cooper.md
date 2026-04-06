# What is Cooper?

Cooper is a backend framework for TypeScript with a Rust runtime. You write your services in TypeScript using Cooper's SDK. Underneath, a Rust binary handles HTTP serving, request routing, connection pooling, and infrastructure provisioning.

## Why Cooper?

| | Express/Fastify | Encore | Cooper |
|---|---|---|---|
| Language | TypeScript | TypeScript | TypeScript |
| Runtime | Node.js | Go | **Rust** |
| Local infra | Docker Compose | Built-in | **Built-in** |
| Deploy | DIY | Encore Cloud | **Your cloud** |
| Middleware | Flexible | Restricted | **Flexible** |
| Lock-in | None | Encore account | **None** |

Cooper gives you Encore-level DX (declare your infra in code, one-command deploy) without the lock-in. Your app deploys to **your own AWS/GCP/Azure/Fly account** — no Cooper account needed.

## How it works

```
  Your TypeScript Code
         │
         ▼
  ┌──────────────────────┐
  │  Cooper Runtime      │
  │  (Rust binary)       │
  │                      │
  │  • Hyper 1.x HTTP    │
  │  • Tokio async       │
  │  • JS Worker Pool    │
  │  • Static analysis   │
  └──────────────────────┘
         │
         ▼
  ┌──────────────────────┐
  │  Infrastructure      │
  │  Postgres │ NATS     │
  │  Valkey   │ S3       │
  └──────────────────────┘
```

1. You write `api()`, `database()`, `topic()`, etc. using the SDK
2. Cooper's Rust binary analyzes your TypeScript at startup — no runtime reflection
3. It starts embedded Postgres, NATS, and Valkey locally (no Docker)
4. HTTP requests are handled by Rust, dispatched to a pool of JS workers
5. On deploy, Cooper reads your declarations and provisions cloud resources

## Key principles

- **No magic** — every API is an explicit function call
- **No lock-in** — deploys to your cloud, standard Postgres, standard everything
- **No boilerplate** — middleware, auth, validation are composable, not restrictive
- **No Docker for dev** — `cooper run` starts everything
