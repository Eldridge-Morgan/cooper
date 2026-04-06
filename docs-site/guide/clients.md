# Generated Clients

Cooper generates fully typed API clients from your route definitions.

## Generate

```bash
cooper gen client --lang typescript    # for your frontend
cooper gen client --lang python        # for scripts/ML pipelines
cooper gen client --lang rust          # for other Rust services
cooper gen openapi                     # OpenAPI 3.1 spec
cooper gen postman                     # Postman collection
```

## TypeScript client

```ts
import { CooperClient } from "~gen/client";

const api = new CooperClient({
  baseUrl: "https://api.myapp.com",
  token: userToken,
});

const { user } = await api.getUser("u_123");
const { posts } = await api.listPosts();
const { post } = await api.createPost({ title: "Hello", body: "World" });
```

Every method is typed — parameters, return values, everything matches the server.

## Service-to-service

For calls between Cooper services in a monorepo:

```ts
import { UsersService } from "~gen/clients/users";

const { user } = await UsersService.getUser({ id: principal.userId });
```

Auth is forwarded automatically. Calls are traced.
