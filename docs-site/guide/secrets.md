# Secrets

Secrets fetched at runtime — never in `.env`, never in source code.

```ts
import { secret } from "cooper/secrets";

const stripeKey = secret("stripe-api-key");

// Fetched at runtime from vault
const client = new Stripe(await stripeKey());
```

## Set secrets

```bash
cooper secrets set stripe-api-key --env prod
cooper secrets set stripe-api-key --env local
cooper secrets ls --env prod
cooper secrets rm stripe-api-key --env prod
```
