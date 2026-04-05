import { page } from "cooper/ssr";

export default page(async () => {
  return (
    <div>
      <h1>Blog Posts</h1>
      <p>Server-rendered post listing.</p>
      <ul>
        <li><a href="/posts/1">Post 1</a></li>
        <li><a href="/posts/2">Post 2</a></li>
      </ul>
    </div>
  );
});
