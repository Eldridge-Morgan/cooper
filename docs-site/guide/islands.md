# Islands

Zero JS ships to the browser unless you declare an island. Islands are components that hydrate on the client.

## Define an island

```tsx
// islands/LikeButton.island.tsx
import { island } from "cooper-stack/islands";
import { useState } from "react";

export default island(function LikeButton({ userId, initialCount }) {
  const [count, setCount] = useState(initialCount);
  return <button onClick={() => setCount(c => c + 1)}>Like ({count})</button>;
});
```

## Use in a page

```tsx
import LikeButton from "~/islands/LikeButton.island";

export default page(async ({ params }) => {
  return (
    <div>
      <h1>Post Title</h1>           {/* pure HTML, zero JS */}
      <p>Post content...</p>         {/* pure HTML, zero JS */}
      <LikeButton                    {/* JS ships only for this */}
        hydrate="visible"
        userId={params.id}
        initialCount={42}
      />
    </div>
  );
});
```

## Hydration strategies

| Strategy | When JS loads |
|---|---|
| `load` | Immediately (default) |
| `visible` | When scrolled into viewport |
| `idle` | When browser is idle |
| `interaction` | On first click/focus/hover on the island |
| `none` | Never — static HTML only |
