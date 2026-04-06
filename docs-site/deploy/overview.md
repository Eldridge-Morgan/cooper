# Deployment

Cooper deploys to your own cloud account. No Cooper account needed.

## Supported clouds

| Cloud | Compute | Database | Cache | Messaging | Storage |
|---|---|---|---|---|---|
| **AWS** | ECS Fargate | RDS | ElastiCache | SNS/SQS | S3 |
| **GCP** | Cloud Run | Cloud SQL | Memorystore | Pub/Sub | GCS |
| **Azure** | Container Apps | Azure DB | Azure Redis | Service Bus | Blob Storage |
| **Fly.io** | Fly Machines | Fly Postgres | Upstash Redis | Upstash QStash | Fly Volumes |

## Deploy

```bash
# See what will be created + cost estimate
cooper deploy --env prod --cloud aws --dry-run

# Actually provision and deploy
cooper deploy --env prod --cloud aws
```

## Environments

```bash
cooper env ls           # list all environments
cooper env url prod     # get the URL
cooper destroy --env staging  # tear down
```

## Preview environments

```yaml
# .github/workflows/preview.yml
- run: |
    cooper deploy \
      --env preview-pr-${{ github.event.number }} \
      --cloud aws \
      --auto-destroy-after 48h
```

Each PR gets its own isolated database, cache, and compute. Destroys itself after 48h.
