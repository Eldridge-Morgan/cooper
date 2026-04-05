use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use futures::stream::Stream;
use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;

/// SSE stream that receives events from the JS runtime.
pub struct SseStream {
    rx: mpsc::Receiver<SseEvent>,
}

pub struct SseEvent {
    pub event_type: Option<String>,
    pub data: String,
    pub id: Option<String>,
}

impl SseStream {
    pub fn new(rx: mpsc::Receiver<SseEvent>) -> Self {
        Self { rx }
    }
}

impl Stream for SseStream {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.rx.poll_recv(cx) {
            Poll::Ready(Some(event)) => {
                let mut sse_event = Event::default().data(event.data);
                if let Some(t) = event.event_type {
                    sse_event = sse_event.event(t);
                }
                if let Some(id) = event.id {
                    sse_event = sse_event.id(id);
                }
                Poll::Ready(Some(Ok(sse_event)))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Create an SSE response from a channel.
pub fn sse_response(rx: mpsc::Receiver<SseEvent>) -> impl IntoResponse {
    let stream = SseStream::new(rx);
    Sse::new(stream).keep_alive(KeepAlive::default())
}
