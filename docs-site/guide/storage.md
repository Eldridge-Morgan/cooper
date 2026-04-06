# Object Storage

File storage backed by the local filesystem in dev and S3/GCS/Azure Blob in production.

```ts
import { bucket } from "cooper/storage";

export const avatars = bucket("avatars", { public: true });

await avatars.upload("user-123/avatar.png", buffer, { contentType: "image/png" });
const stream = await avatars.download("user-123/avatar.png");
const url = await avatars.signedUrl("user-123/avatar.png", { ttl: "15m" });
await avatars.delete("user-123/avatar.png");
const files = await avatars.list("user-123/");
```
