//! The traceback of a program that wasmtime stopped with a trap.

use crate::Module;
use crate::cpython::{
    MEMORY_ERROR, MORE_TIME, RECURSIVE_CUTOFF, REPEATED, REPEATED_END, TRACEBACK,
};
use crate::ir::SiteFrame;

/// What wasmtime writes for an allocation larger than the GC heap holds,
/// and for one larger than any heap holds.
const OUT_OF_MEMORY: [&str; 2] = ["GC heap out of memory", "allocation size too large"];

impl Module {
    /// The traceback that CPython writes for a `MemoryError`, for the
    /// standard error of a wasmtime run that stopped because the program
    /// asked for more memory than its GC heap holds.
    ///
    /// Returns `None` for any other error, when a frame stopped in code
    /// without a site, and when the backtrace lacks the frame of the
    /// module. The code that passes an exception on to the caller has no
    /// site, and neither has the code that builds the literals. Wasmtime
    /// keeps 20 frames of the backtrace unless `-D max-backtrace` asks for
    /// more.
    pub fn memory_error(&self, stderr: &str) -> Option<String> {
        // What the program wrote comes first, and no line of it starts with
        // `Error: `, which starts the error of wasmtime.
        let wasmtime_error = stderr
            .rsplit_once("\nError: ")
            .map(|(_, error)| error)
            .or_else(|| stderr.strip_prefix("Error: "))?;
        if !OUT_OF_MEMORY
            .iter()
            .any(|trap| wasmtime_error.contains(trap))
        {
            return None;
        }
        let mut frames = Vec::new();
        let mut outermost = None;
        for offset in backtrace(wasmtime_error) {
            // A function of the runtime has no frame in the traceback.
            if !self.map.frames.iter().any(|code| code.contains(&offset)) {
                continue;
            }
            let site = self
                .map
                .sites
                .iter()
                .filter(|range| range.start <= offset && offset < range.end)
                .max_by_key(|range| range.start)?
                .site;
            frames.push(self.sites.get(site.0 as usize)?);
            outermost = Some(offset);
        }
        if !self.map.module.contains(&outermost?) {
            return None;
        }
        frames.reverse();
        Some(traceback(&frames, MEMORY_ERROR))
    }
}

/// The offset of each frame of the wasm backtrace in `error`, from the
/// innermost frame, as `0x70e` in `    1:    0x70e - <unknown>!<wasm
/// function 26>`. The innermost frame has the offset of the instruction
/// that trapped, and every other frame the offset of its call.
fn backtrace(error: &str) -> impl Iterator<Item = u32> {
    error.lines().filter_map(|line| {
        let (number, rest) = line.trim_start().split_once(':')?;
        number.parse::<u32>().ok()?;
        let (offset, _) = rest.trim_start().strip_prefix("0x")?.split_once(" - ")?;
        u32::from_str_radix(offset, 16).ok()
    })
}

/// The traceback of `frames`, from the outermost, as the module writes it
/// for an exception that nothing catches.
fn traceback(frames: &[&SiteFrame], message: &str) -> String {
    let cutoff = usize::try_from(RECURSIVE_CUTOFF).expect("a positive cutoff");
    let mut text = String::from(TRACEBACK);
    let mut last = None;
    let mut count = 0;
    for frame in frames {
        if last != Some(frame.line) {
            repeated(&mut text, count, cutoff);
            last = Some(frame.line);
            count = 0;
        }
        count += 1;
        if count <= cutoff {
            text.push_str(&frame.text);
        }
    }
    repeated(&mut text, count, cutoff);
    text.push_str(message);
    text.push('\n');
    text
}

/// Adds how many frames of a run the traceback leaves out, when the run has
/// more than `cutoff`.
fn repeated(text: &mut String, count: usize, cutoff: usize) {
    if count > cutoff {
        let more = count - cutoff;
        let s = if more > 1 { "s" } else { "" };
        text.push_str(&format!("{REPEATED}{more}{MORE_TIME}{s}{REPEATED_END}"));
    }
}
