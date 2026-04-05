import { page } from "cooper/ssr";

export default page(async () => {
  return (
    <div>
      <h1>Welcome to Cooper</h1>
      <p>Your backend is running. Try these endpoints:</p>
      <ul>
        <li><code>GET /health</code> — health check</li>
        <li><code>GET /users</code> — list users</li>
        <li><code>POST /users</code> — create a user</li>
        <li><code>GET /users/:id</code> — get a user</li>
        <li><code>GET /_cooper/info</code> — API info</li>
      </ul>
    </div>
  );
});
