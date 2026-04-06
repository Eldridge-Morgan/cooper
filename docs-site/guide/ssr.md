# SSR & Pages

Cooper supports server-rendered pages with file-based routing.

## Pages

```
pages/
  index.tsx              → /
  about.tsx              → /about
  users/[id].tsx         → /users/:id
  blog/[...slug].tsx     → /blog/* (catch-all)
  _layout.tsx            → wraps all pages
```

```tsx
import { page } from "cooper-stack/ssr";

export default page(async ({ params }) => {
  const user = await UsersService.getUser({ id: params.id });
  return (
    <div>
      <h1>{user.name}</h1>
      <p>{user.email}</p>
    </div>
  );
});
```

## Layouts

```tsx
import { layout } from "cooper-stack/ssr";

export default layout(({ children }) => (
  <div>
    <Nav />
    <main>{children}</main>
    <Footer />
  </div>
));
```

## Data loading

```tsx
import { page, pageLoader } from "cooper-stack/ssr";

export const loader = pageLoader(async ({ params }) => {
  const user = await UsersService.getUser({ id: params.id });
  return { user };
});

export default page(async ({ data }) => {
  return <h1>{data.user.name}</h1>;
});
```

## Route priority

When an API route and a page route share the same path, the API route takes priority.
