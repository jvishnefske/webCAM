//! Async block trait — forward declaration for Phase 2 channel/HAL work.
//!
//! Phase 1 introduces the CSP channel substrate and the multi-hop mesh
//! router (both in the `pubsub` crate). Block authors that need to drive
//! typed channels or embedded-hal-async peripherals from a cooperative
//! loop will implement [`AsyncBlock`] in Phase 2, once the `BlockContext`
//! exposes the channel and peripheral handles.
//!
//! This module is deliberately small: the trait exists so downstream
//! crates can begin referring to it, but the context carries no fields
//! yet. Adding fields is a backwards-compatible change because
//! [`BlockContext`] is `#[non_exhaustive]`.

use core::marker::PhantomData;

/// A block that runs a cooperative async loop.
///
/// The `run` method is expected to loop indefinitely, awaiting on channels
/// or peripheral events inside the body. Returning early is treated as the
/// block exiting cleanly.
#[allow(async_fn_in_trait)]
pub trait AsyncBlock {
    /// Run the block to completion (typically never returns).
    async fn run(&mut self, ctx: &mut BlockContext<'_>);
}

/// Runtime-provided context for an [`AsyncBlock`].
///
/// Reserved for Phase 2 fields: a `&mut dyn NodeApi` façade over
/// `pubsub::Node` so blocks can build `MarshalledChannel`s, and a
/// `&mut dyn AsyncPeripherals` surface for `embedded-hal-async` access.
#[non_exhaustive]
pub struct BlockContext<'a> {
    _marker: PhantomData<&'a ()>,
}

impl<'a> BlockContext<'a> {
    /// Build an empty context. Present as a Phase 1 stub; Phase 2 will add
    /// constructors that take the real channel and peripheral handles.
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<'a> Default for BlockContext<'a> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counter {
        n: u32,
    }

    impl AsyncBlock for Counter {
        async fn run(&mut self, _ctx: &mut BlockContext<'_>) {
            self.n += 1;
        }
    }

    #[test]
    fn async_block_can_be_invoked_statically() {
        use core::future::Future;
        use core::pin::pin;
        use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

        const VTABLE: RawWakerVTable = RawWakerVTable::new(
            |_| RawWaker::new(core::ptr::null(), &VTABLE),
            |_| {},
            |_| {},
            |_| {},
        );

        let mut c = Counter { n: 0 };
        {
            let mut ctx = BlockContext::new();
            let fut = c.run(&mut ctx);
            // SAFETY: the vtable is no-op and the null data pointer is never
            // dereferenced, so constructing a Waker from it is sound here.
            let waker = unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) };
            let mut cx = Context::from_waker(&waker);
            let mut pinned = pin!(fut);
            assert!(matches!(pinned.as_mut().poll(&mut cx), Poll::Ready(())));
        }
        assert_eq!(c.n, 1);
    }

    #[test]
    fn block_context_default_matches_new() {
        let _a: BlockContext<'_> = BlockContext::new();
        let _b: BlockContext<'_> = BlockContext::default();
    }
}
