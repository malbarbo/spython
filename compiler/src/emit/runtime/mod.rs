//! The code that every module carries whatever the program does: the
//! exceptions with their traceback, the depth of the calls, and the calls
//! to the host.

pub(super) mod exc;
pub(super) mod host;
