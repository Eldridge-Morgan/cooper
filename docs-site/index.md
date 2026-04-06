---
layout: home
hero:
  name: " "
  text: The backend framework for TypeScript
  tagline: Write TypeScript. Run on Rust. Deploy anywhere.
  image:
    src: /logo.svg
    alt: Cooper
  actions:
    - theme: brand
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: View on GitHub
      link: https://github.com/Eldridge-Morgan/cooper
features:
  - title: Write TypeScript, Run Rust
    details: Your handlers are TypeScript. The runtime underneath is Rust — Hyper 1.x, Tokio, Axum. 8-10x faster than Express, ~10ms cold starts.
  - title: Zero-Config Local Dev
    details: "cooper run starts embedded Postgres, NATS, and Valkey. Runs migrations. Hot reloads. No Docker required."
  - title: Deploy Anywhere
    details: One command deploys to AWS, GCP, Azure, or Fly.io. Cooper provisions all infrastructure — databases, queues, cache, compute.
  - title: Type-Safe Everything
    details: Auto-generated typed clients for your frontend. OpenAPI spec from your code. Zod validation runs before your handler.
  - title: Batteries Included
    details: Database, cache, pub/sub, queues, cron, object storage, secrets, auth — all built in. No glue code.
  - title: Live Dashboard
    details: Service map, API explorer, request log — all in a minimal monochrome UI powered by dagre.
---
