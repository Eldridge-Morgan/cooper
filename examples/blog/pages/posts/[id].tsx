import { page, pageLoader, Suspense } from "cooper/ssr";

export default page(async ({ params }) => {
  return (
    <div>
      <h1>Post {params.id}</h1>
      <p>This page is server-rendered by Cooper.</p>
      <a href="/posts">← Back to posts</a>
    </div>
  );
});
