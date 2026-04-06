# Deploy to Fly.io

## Prerequisites

- `flyctl` installed and authenticated
- `FLY_API_TOKEN` set (or `flyctl auth login`)

## Deploy

```bash
cooper deploy --env prod --cloud fly
```

## What gets created

| Resource | Spec |
|---|---|
| Fly Postgres | shared-cpu-1x, 1 GB |
| Upstash Redis | No replicas |
| Fly Volume | 1 GB for storage |
| Fly Machine | shared-cpu-1x, 256 MB |

Cooper generates a `fly.toml` automatically.

## Cost estimate

```
Estimated monthly delta: ~$0/mo (free tier covers most)
```

Fly.io's free tier includes 3 shared VMs and 1 Postgres cluster.
