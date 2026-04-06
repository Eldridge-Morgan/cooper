# Streaming

Cooper supports Server-Sent Events (SSE) and WebSocket routes.

## SSE

```ts
export const liveEvents = api(
  { method: "GET", path: "/events/:roomId", stream: "sse" },
  async function* ({ roomId }) {
    while (true) {
      const event = await waitForEvent(roomId);
      yield { type: event.type, data: event.payload };
    }
  }
);
```

## WebSocket

```ts
export const chatSocket = api(
  { path: "/ws/chat", stream: "websocket" },
  async (socket) => {
    for await (const msg of socket) {
      await socket.send({ echo: msg.data, ts: Date.now() });
    }
  }
);
```
