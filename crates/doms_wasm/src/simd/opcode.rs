//! WebAssembly SIMD (v128) opcodes and binary bytecode decoder.

#[derive(Debug, Clone, PartialEq)]
pub enum WasmSimdOpcode {
    /// `v128.load offset: u32`
    V128Load { offset: u32 },
    /// `v128.store offset: u32`
    V128Store { offset: u32 },
    /// `v128.const bytes: [u8; 16]`
    V128Const([u8; 16]),
    /// `i8x16.shuffle lanes: [u8; 16]`
    I8x16Shuffle([u8; 16]),

    // Splats
    I8x16Splat,
    I16x8Splat,
    I32x4Splat,
    I64x2Splat,
    F32x4Splat,
    F64x2Splat,

    // Lane extraction & replacement
    I8x16ExtractLaneS(u8),
    I8x16ExtractLaneU(u8),
    I8x16ReplaceLane(u8),
    I16x8ExtractLaneS(u8),
    I16x8ExtractLaneU(u8),
    I16x8ReplaceLane(u8),
    I32x4ExtractLane(u8),
    I32x4ReplaceLane(u8),
    I64x2ExtractLane(u8),
    I64x2ReplaceLane(u8),
    F32x4ExtractLane(u8),
    F32x4ReplaceLane(u8),
    F64x2ExtractLane(u8),
    F64x2ReplaceLane(u8),

    // Bitwise & select
    V128Not,
    V128And,
    V128AndNot,
    V128Or,
    V128Xor,
    V128BitSelect,
    V128AnyTrue,

    // Integer arithmetic (i8x16)
    I8x16Add,
    I8x16Sub,
    I8x16AddSatS,
    I8x16AddSatU,
    I8x16SubSatS,
    I8x16SubSatU,
    I8x16MinS,
    I8x16MinU,
    I8x16MaxS,
    I8x16MaxU,
    I8x16Eq,
    I8x16Ne,

    // Integer arithmetic (i16x8)
    I16x8Add,
    I16x8Sub,
    I16x8Mul,
    I16x8MinS,
    I16x8MinU,
    I16x8MaxS,
    I16x8MaxU,
    I16x8Eq,
    I16x8Ne,

    // Integer arithmetic (i32x4)
    I32x4Add,
    I32x4Sub,
    I32x4Mul,
    I32x4MinS,
    I32x4MinU,
    I32x4MaxS,
    I32x4MaxU,
    I32x4DotI16x8S,
    I32x4Eq,
    I32x4Ne,

    // Integer arithmetic (i64x2)
    I64x2Add,
    I64x2Sub,
    I64x2Mul,
    I64x2Eq,
    I64x2Ne,

    // Floating-point arithmetic (f32x4)
    F32x4Abs,
    F32x4Neg,
    F32x4Sqrt,
    F32x4Add,
    F32x4Sub,
    F32x4Mul,
    F32x4Div,
    F32x4Min,
    F32x4Max,
    F32x4Eq,
    F32x4Ne,
    F32x4Lt,
    F32x4Gt,

    // Floating-point arithmetic (f64x2)
    F64x2Abs,
    F64x2Neg,
    F64x2Sqrt,
    F64x2Add,
    F64x2Sub,
    F64x2Mul,
    F64x2Div,
    F64x2Min,
    F64x2Max,
    F64x2Eq,
    F64x2Ne,
    F64x2Lt,
    F64x2Gt,
}

/// Decode a sequence of Wasm SIMD (v128) bytecodes.
///
/// In standard WebAssembly, SIMD instructions are prefixed with `0xFD`
/// followed by an unsigned LEB128 opcode and optional immediate operands.
pub fn decode_simd_bytecode(bytes: &[u8]) -> Result<Vec<WasmSimdOpcode>, String> {
    let mut opcodes = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let prefix = bytes[i];
        i += 1;
        if prefix != 0xFD {
            return Err(format!("Expected 0xFD SIMD prefix at byte offset {}", i - 1));
        }

        if i >= bytes.len() {
            return Err("Unexpected end of bytecode after 0xFD prefix".into());
        }

        // Read LEB128 opcode
        let mut opcode_val: u32 = 0;
        let mut shift = 0;
        loop {
            if i >= bytes.len() {
                return Err("Unexpected EOF while reading LEB128 opcode".into());
            }
            let byte = bytes[i];
            i += 1;
            opcode_val |= ((byte & 0x7F) as u32) << shift;
            if (byte & 0x80) == 0 {
                break;
            }
            shift += 7;
            if shift >= 32 {
                return Err("LEB128 opcode overflow".into());
            }
        }

        let op = match opcode_val {
            0x00 => {
                // v128.load: memarg = alignment (LEB128) + offset (LEB128)
                let _align = read_u32_leb128(bytes, &mut i)?;
                let offset = read_u32_leb128(bytes, &mut i)?;
                WasmSimdOpcode::V128Load { offset }
            }
            0x0B => {
                // v128.store: memarg
                let _align = read_u32_leb128(bytes, &mut i)?;
                let offset = read_u32_leb128(bytes, &mut i)?;
                WasmSimdOpcode::V128Store { offset }
            }
            0x0C => {
                // v128.const: 16 bytes
                if i + 16 > bytes.len() {
                    return Err("Unexpected EOF while reading v128.const immediate".into());
                }
                let mut data = [0u8; 16];
                data.copy_from_slice(&bytes[i..i + 16]);
                i += 16;
                WasmSimdOpcode::V128Const(data)
            }
            0x0D => {
                // i8x16.shuffle: 16 lane indices
                if i + 16 > bytes.len() {
                    return Err("Unexpected EOF while reading i8x16.shuffle lanes".into());
                }
                let mut lanes = [0u8; 16];
                lanes.copy_from_slice(&bytes[i..i + 16]);
                i += 16;
                WasmSimdOpcode::I8x16Shuffle(lanes)
            }
            0x0F => WasmSimdOpcode::I8x16Splat,
            0x10 => WasmSimdOpcode::I16x8Splat,
            0x11 => WasmSimdOpcode::I32x4Splat,
            0x12 => WasmSimdOpcode::I64x2Splat,
            0x13 => WasmSimdOpcode::F32x4Splat,
            0x14 => WasmSimdOpcode::F64x2Splat,

            0x15 => WasmSimdOpcode::I8x16ExtractLaneS(read_lane_idx(bytes, &mut i)?),
            0x16 => WasmSimdOpcode::I8x16ExtractLaneU(read_lane_idx(bytes, &mut i)?),
            0x17 => WasmSimdOpcode::I8x16ReplaceLane(read_lane_idx(bytes, &mut i)?),
            0x18 => WasmSimdOpcode::I16x8ExtractLaneS(read_lane_idx(bytes, &mut i)?),
            0x19 => WasmSimdOpcode::I16x8ExtractLaneU(read_lane_idx(bytes, &mut i)?),
            0x1A => WasmSimdOpcode::I16x8ReplaceLane(read_lane_idx(bytes, &mut i)?),
            0x1B => WasmSimdOpcode::I32x4ExtractLane(read_lane_idx(bytes, &mut i)?),
            0x1C => WasmSimdOpcode::I32x4ReplaceLane(read_lane_idx(bytes, &mut i)?),
            0x1D => WasmSimdOpcode::I64x2ExtractLane(read_lane_idx(bytes, &mut i)?),
            0x1E => WasmSimdOpcode::I64x2ReplaceLane(read_lane_idx(bytes, &mut i)?),
            0x1F => WasmSimdOpcode::F32x4ExtractLane(read_lane_idx(bytes, &mut i)?),
            0x20 => WasmSimdOpcode::F32x4ReplaceLane(read_lane_idx(bytes, &mut i)?),
            0x21 => WasmSimdOpcode::F64x2ExtractLane(read_lane_idx(bytes, &mut i)?),
            0x22 => WasmSimdOpcode::F64x2ReplaceLane(read_lane_idx(bytes, &mut i)?),

            0x23 => WasmSimdOpcode::I8x16Eq,
            0x24 => WasmSimdOpcode::I8x16Ne,
            0x2D => WasmSimdOpcode::I16x8Eq,
            0x2E => WasmSimdOpcode::I16x8Ne,
            0x37 => WasmSimdOpcode::I32x4Eq,
            0x38 => WasmSimdOpcode::I32x4Ne,
            0x41 => WasmSimdOpcode::F32x4Eq,
            0x42 => WasmSimdOpcode::F32x4Ne,
            0x43 => WasmSimdOpcode::F32x4Lt,
            0x44 => WasmSimdOpcode::F32x4Gt,
            0x47 => WasmSimdOpcode::F64x2Eq,
            0x48 => WasmSimdOpcode::F64x2Ne,
            0x49 => WasmSimdOpcode::F64x2Lt,
            0x4A => WasmSimdOpcode::F64x2Gt,

            0x4D => WasmSimdOpcode::V128Not,
            0x4E => WasmSimdOpcode::V128And,
            0x4F => WasmSimdOpcode::V128AndNot,
            0x50 => WasmSimdOpcode::V128Or,
            0x51 => WasmSimdOpcode::V128Xor,
            0x52 => WasmSimdOpcode::V128BitSelect,
            0x53 => WasmSimdOpcode::V128AnyTrue,

            0x60 => WasmSimdOpcode::F32x4Abs,
            0x61 => WasmSimdOpcode::F32x4Neg,
            0x63 => WasmSimdOpcode::F32x4Sqrt,
            0x67 => WasmSimdOpcode::F32x4Add,
            0x68 => WasmSimdOpcode::F32x4Sub,
            0x69 => WasmSimdOpcode::F32x4Mul,
            0x6A => WasmSimdOpcode::F32x4Div,
            0x6B => WasmSimdOpcode::F32x4Min,
            0x6C => WasmSimdOpcode::F32x4Max,

            0x70 => WasmSimdOpcode::F64x2Abs,
            0x71 => WasmSimdOpcode::F64x2Neg,
            0x73 => WasmSimdOpcode::F64x2Sqrt,
            0x7A => WasmSimdOpcode::F64x2Add,
            0x7B => WasmSimdOpcode::F64x2Sub,
            0x7C => WasmSimdOpcode::F64x2Mul,
            0x7D => WasmSimdOpcode::F64x2Div,
            0x7E => WasmSimdOpcode::F64x2Min,
            0x7F => WasmSimdOpcode::F64x2Max,

            0x6E => WasmSimdOpcode::I8x16Add,
            0x6F => WasmSimdOpcode::I8x16Sub,
            0x80 => WasmSimdOpcode::I16x8Add,
            0x81 => WasmSimdOpcode::I16x8Sub,
            0x85 => WasmSimdOpcode::I16x8Mul,
            0x8E => WasmSimdOpcode::I32x4Add,
            0x91 => WasmSimdOpcode::I32x4Sub,
            0x95 => WasmSimdOpcode::I32x4Mul,
            0x9E => WasmSimdOpcode::I64x2Add,
            0xA1 => WasmSimdOpcode::I64x2Sub,
            0xA5 => WasmSimdOpcode::I64x2Mul,
            0xBA => WasmSimdOpcode::I32x4DotI16x8S,

            unknown => return Err(format!("Unsupported Wasm SIMD opcode 0x{:X}", unknown)),
        };

        opcodes.push(op);
    }

    Ok(opcodes)
}

fn read_u32_leb128(bytes: &[u8], i: &mut usize) -> Result<u32, String> {
    let mut val: u32 = 0;
    let mut shift = 0;
    loop {
        if *i >= bytes.len() {
            return Err("Unexpected EOF reading LEB128 immediate".into());
        }
        let byte = bytes[*i];
        *i += 1;
        val |= ((byte & 0x7F) as u32) << shift;
        if (byte & 0x80) == 0 {
            break;
        }
        shift += 7;
        if shift >= 32 {
            return Err("LEB128 overflow".into());
        }
    }
    Ok(val)
}

fn read_lane_idx(bytes: &[u8], i: &mut usize) -> Result<u8, String> {
    if *i >= bytes.len() {
        return Err("Unexpected EOF reading lane index".into());
    }
    let idx = bytes[*i];
    *i += 1;
    Ok(idx)
}
