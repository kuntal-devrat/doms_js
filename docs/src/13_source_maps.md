# Source Maps & Production Stack Traces

Production JavaScript is frequently bundled, minified, or compiled from TypeScript. DOMS.js integrates a native **Source Map v3** engine with Base64-VLQ decoding to map runtime errors directly back to original TypeScript/authored source files.

---

## 1. Source Map v3 Specification

A standard source map provides:
```json
{
  "version": 3,
  "file": "out.js",
  "sources": ["src/app.ts"],
  "names": ["computeTotal"],
  "mappings": "AAAA,SAAS..."
}
```

DOMS decodes Variable-Length Quantity (VLQ) segments into structured `Mapping` records:
1. Generated column delta
2. Original source file index delta
3. Original line number delta
4. Original column number delta
5. Original symbol name index delta (optional)

---

## 2. Fast Position Lookup & Stack Symbolication

```rust
use doms_compiler::{SourceMap, decode_vlq};

let source_map = SourceMap::parse_json(raw_map_json).expect("valid map");

// Lookup original file & line for a runtime crash
if let Some(pos) = source_map.lookup(generated_line, generated_column) {
    println!("Error at {}:{}:{}", pos.source, pos.line, pos.column);
}

// Automatic frame symbolication
let raw_frame = "    at compute (bundle.js:1:254)";
let symbolicated = source_map.symbolicate_frame(raw_frame);
// Outputs: "    at compute (from computeTotal) (src/app.ts:42:15)"
```
