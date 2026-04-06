# Deploy to GCP

## Prerequisites

- `gcloud` CLI authenticated
- Project ID set (`GOOGLE_CLOUD_PROJECT` or `gcloud config set project`)

## Deploy

```bash
cooper deploy --env prod --cloud gcp
```

## What gets created

| Resource | Spec |
|---|---|
| Cloud SQL | db-f1-micro, Postgres 15 |
| Memorystore | 1 GB Redis |
| Pub/Sub topics | Per topic declaration |
| GCS bucket | Standard storage |
| Cloud Run | Auto-scaling, 512 MB |

## Cost estimate

```
+ Cloud SQL          ~$10/mo
+ Memorystore        ~$10/mo
+ Cloud Run          ~$0/mo (pay per use)
```
