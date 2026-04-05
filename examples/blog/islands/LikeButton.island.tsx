import { island } from "cooper/islands";
import { useState } from "react";

export default island(function LikeButton({
  postId,
  initialCount,
}: {
  postId: string;
  initialCount: number;
}) {
  const [count, setCount] = useState(initialCount);
  const [liked, setLiked] = useState(false);

  const toggle = async () => {
    const res = await fetch(`/api/posts/${postId}/like`, {
      method: liked ? "DELETE" : "POST",
    });
    const data = await res.json();
    setCount(data.count);
    setLiked(!liked);
  };

  return (
    <button onClick={toggle}>
      {liked ? "♥ Unlike" : "♡ Like"} ({count})
    </button>
  );
});
