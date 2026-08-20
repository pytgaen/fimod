//! Allocator-backed memory accounting for the Monty sandbox.
//!
//! Since Monty 0.0.20 the interpreter no longer counts heap bytes itself: its
//! `ResourceTracker` reads `LIVE_MEMORY - BASELINE_MEMORY`, two globals in
//! `monty-types` that only a charging global allocator ever writes. Upstream
//! ships one (`monty-alloc`), but it forwards to `System`, and swapping
//! mimalloc out for it costs ~20% on every fimod invocation. So fimod keeps
//! mimalloc and charges the same two counters around it.
//!
//! Without this module `sandbox.max_memory` is silently unenforced: the
//! counters stay at their initial values, `probe_memory()` returns 0 forever,
//! and a mold accumulating memory gradually runs to whatever the host has.
//!
//! Unlike `monty-alloc` this allocator never ends the process on its own. The
//! soft limit is enough: the interpreter checks it at its next checkpoint and
//! raises, which fimod reports as `sandbox exploded: max_memory exceeded`.
//! That matches how the limit behaved before 0.0.20.

#![expect(
    unsafe_code,
    reason = "implementing GlobalAlloc requires unsafe; every method forwards \
              its arguments unchanged to mimalloc and only adds counter arithmetic"
)]

use std::alloc::{GlobalAlloc, Layout};
use std::sync::atomic::Ordering;

use mimalloc::MiMalloc;
use monty_types::{BASELINE_MEMORY, LIVE_MEMORY};

/// mimalloc, plus the live-byte count Monty's resource tracker reads.
pub struct CountingMiMalloc;

// SAFETY: every method forwards its arguments unchanged to `MiMalloc` and
// returns what `MiMalloc` returned. No pointer is fabricated, aliased or freed
// here, so this upholds exactly the invariants `MiMalloc` upholds. The counter
// arithmetic is plain atomics and touches no memory the allocator manages.
unsafe impl GlobalAlloc for CountingMiMalloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE_MEMORY.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: `layout` comes from the caller and is forwarded unchanged.
        unsafe { MiMalloc.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_MEMORY.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: `ptr` came from our `alloc`/`realloc` with this same `layout`.
        unsafe { MiMalloc.dealloc(ptr, layout) };
    }

    // Overridden rather than left to the default (which routes through `alloc`)
    // so mimalloc keeps returning pre-zeroed pages where it can.
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        LIVE_MEMORY.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: `layout` comes from the caller and is forwarded unchanged.
        unsafe { MiMalloc.alloc_zeroed(layout) }
    }

    // Overridden for the same reason: the default reallocates and copies, while
    // mimalloc can often grow a block in place.
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if let Some(growth) = new_size.checked_sub(layout.size()) {
            LIVE_MEMORY.fetch_add(growth, Ordering::Relaxed);
        } else {
            LIVE_MEMORY.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
        }
        // SAFETY: `ptr`/`layout` describe a live block from this allocator, and
        // `new_size` is the caller's — all forwarded unchanged.
        unsafe { MiMalloc.realloc(ptr, layout, new_size) }
    }
}

/// Records what the process costs to exist, so `max_memory` budgets only what
/// molds allocate on top of it.
///
/// Call once, as early in `main` as possible: everything already allocated at
/// that point becomes free from the sandbox's point of view. `fetch_min` means
/// a later, leaner moment can only improve the baseline, never inflate it.
pub fn arm_baseline() {
    let live = LIVE_MEMORY.load(Ordering::Relaxed);
    BASELINE_MEMORY.fetch_min(live, Ordering::Relaxed);
}
