mod basic;

use crate::types::OptimizedOutput;

pub use basic::BasicOptimizer;

pub trait Optimizer {
    fn optimize(&self, raw: &str) -> String;
}

const MAX_OUTPUT_BYTES: usize = 8 * 1024;

pub fn run_pipeline(raw: &[u8], ref_id: &str) -> OptimizedOutput {
    let raw_str = String::from_utf8_lossy(raw);
    let original_bytes = raw.len();

    let optimizer = BasicOptimizer::new();
    let mut optimized = optimizer.optimize(&raw_str);

    let was_truncated = optimized.len() > MAX_OUTPUT_BYTES;
    if was_truncated {
        optimized.truncate(MAX_OUTPUT_BYTES);
        // Find last newline to avoid cutting mid-line
        if let Some(pos) = optimized.rfind('\n') {
            optimized.truncate(pos + 1);
        }
        optimized.push_str(&format!(
            "\n[... truncated. Full output: tkn log {ref_id} ...]"
        ));
    }

    let optimized_bytes = optimized.len();

    OptimizedOutput {
        content: optimized,
        original_bytes,
        optimized_bytes,
        was_truncated,
    }
}
