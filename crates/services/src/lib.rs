//! Common traits and logic for managing the lifecycle of services
#![deny(unused_crate_dependencies)]
#![deny(missing_docs)]

mod service;
mod state;

/// Re-exports for streaming utilities
pub mod stream {
    #[doc(no_inline)]
    pub use futures::stream::{
        pending,
        unfold,
        Stream,
    };

    /// A Send + Sync BoxStream
    pub type BoxStream<T> =
        core::pin::Pin<Box<dyn Stream<Item = T> + Send + Sync + 'static>>;

    /// Helper trait to create a BoxStream from a Stream
    pub trait IntoBoxStream: Stream {
        /// Convert this stream into a BoxStream.
        fn into_boxed(self) -> BoxStream<Self::Item>
        where
            Self: Sized + Send + Sync + 'static,
        {
            Box::pin(self)
        }
    }

    impl<S> IntoBoxStream for S where S: Stream + Send + Sync + 'static {}
}

pub use service::{
    EmptyShared,
    RunnableService,
    RunnableTask,
    Service,
    ServiceRunner,
    Shared,
    SharedMutex,
};
pub use state::{
    State,
    StateWatcher,
};
