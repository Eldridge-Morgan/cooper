# Deploy to AWS

## Prerequisites

- AWS CLI configured (`aws configure`)
- Credentials with permissions for: ECS, RDS, ElastiCache, SQS, SNS, S3, ECR, VPC, EC2

## Deploy

```bash
cooper deploy --env prod --cloud aws
```

## What gets created

| Resource | Service | Spec |
|---|---|---|
| VPC + subnets | Networking | 2 AZs, internet gateway |
| Security group | Firewall | Ports 80, 443, 4000 + internal |
| RDS | Database | db.t3.micro, Postgres 15 |
| ElastiCache | Cache | cache.t3.micro, Redis |
| SNS topics | Pub/Sub | Per topic declaration |
| SQS queues | Queues | Per queue declaration |
| S3 bucket | Storage | Standard |
| ECR repo | Container registry | For app image |
| ECS Fargate | Compute | 256 CPU, 512 MB, auto-scaling |

## Environment variables

Set automatically on the ECS task:
- `COOPER_DB_MAIN_URL` — RDS connection string
- `COOPER_VALKEY_URL` — ElastiCache endpoint
- `COOPER_ENV` — environment name

## Cost estimate

```
+ RDS Postgres (db.t3.micro)     ~$28/mo
+ ElastiCache (cache.t3.micro)   ~$12/mo
+ ECS Fargate                    ~$0/mo (pay per use)
```
