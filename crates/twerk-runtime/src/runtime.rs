use std::pin::Pin;

pub type BoxedFuture<'a, T> = Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;