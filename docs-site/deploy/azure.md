# Deploy to Azure

## Prerequisites

- Azure CLI authenticated (`az login`)
- Subscription active

## Deploy

```bash
cooper deploy --env prod --cloud azure
```

## What gets created

| Resource | Spec |
|---|---|
| Resource group | Groups all resources |
| PostgreSQL Flexible Server | Burstable B1ms |
| Azure Cache for Redis | Basic C0 |
| Service Bus | Basic tier |
| Storage Account | Standard LRS |
| Container App | Auto-scaling, external ingress |

## Cost estimate

```
+ Azure PostgreSQL   ~$15/mo
+ Azure Redis        ~$13/mo
+ Container App      ~$0/mo (pay per use)
```
