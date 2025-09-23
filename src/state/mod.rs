#[cfg(not(feature = "fast-mutex"))]
mod std_state;

#[cfg(not(feature = "fast-mutex"))]
pub(crate) use std_state::SharedState;

#[cfg(feature = "fast-mutex")]
mod parking_lot_state;

#[cfg(feature = "fast-mutex")]
pub(crate) use parking_lot_state::SharedState;
