//! WebAssembly 128-bit SIMD Vector Extensions.

pub mod compiler;
pub mod engine;
pub mod opcode;

pub use compiler::{NativeSimdFn, WasmSimdCompiler, WasmVectorKernel};
pub use engine::{WasmSimdEngine, register_wasm_simd};
pub use opcode::{WasmSimdOpcode, decode_simd_bytecode};
