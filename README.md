# hwpx-filler

Universal HWPX form-filling engine — runs entirely in the browser.  
No server. No data upload. Your personal information never leaves your device.

**[Live Demo →](https://merryai-dev.github.io/hwpx-filler/)**

---

## What it does

Korean government forms (HWPX) are painful to fill repeatedly — especially when the same person needs to submit 10+ variants of the same document.

hwpx-filler lets you:
1. Upload your source data (filled HWPX, CSV, or JSON)
2. Upload the blank target form
3. AI maps the fields automatically (bring your own Anthropic API key)
4. Download the filled form — format preserved, byte-perfect

Everything runs in the browser via WebAssembly. No backend, no tracking.

---

## Usage

### Option A: Use the hosted wizard

Open [the demo](https://merryai-dev.github.io/hwpx-filler/) in your browser.  
Enter your Anthropic API key once (stored in localStorage, never sent anywhere except Anthropic's API).

### Option B: Use the Rust library

```toml
# Cargo.toml
hwpx-filler = "0.1"
```

```rust
use hwpx_filler::{zipper, stream_analyzer, patcher};

let bytes = std::fs::read("form.hwpx")?;
let text_files = zipper::extract_text_files(&bytes)?;
let section0 = &text_files["Contents/section0.xml"];

let tables = stream_analyzer::analyze_xml(section0);
let fields = stream_analyzer::extract_fields(&tables);
```

### Option C: Use the npm package

```bash
npm install hwpx-filler
```

```js
import init, { analyzeForm, fillForm } from 'hwpx-filler';
await init();

const fields = JSON.parse(analyzeForm(hwpxBytes).json);
const filled = fillForm(hwpxBytes, JSON.stringify(patches));
```

---

## Architecture

```
hwpx-filler/
├── src/
│   ├── stream_analyzer.rs  — streaming XML parser, table structure detection
│   ├── patcher.rs          — byte-level cell patching, row cloning
│   ├── extractor.rs        — label:value pair extraction
│   ├── llm_format.rs       — table → LLM-readable text
│   ├── zipper.rs           — binary-safe ZIP patch (no repack)
│   └── wasm.rs             — 7 WASM exports
└── wizard.html             — 4-step browser UI (no build step)
```

**Key design decisions:**
- No regex — quick-xml + serde for type-safe parsing
- Patch original ZIP, don't repack — preserves embedded images byte-perfect
- Two-pass patcher: find cell positions first, then string-replace (handles HWPX's cellAddr ordering quirk)
- LLM-first mapping (Claude structured outputs) with rules-based fallback

---

## WASM API

| Export | Description |
|--------|-------------|
| `analyzeForm(bytes)` | Extract table structure + field candidates |
| `fillForm(bytes, patches)` | Apply cell patches → filled HWPX bytes |
| `cloneRows(bytes, clones)` | Duplicate table rows (for career/history tables) |
| `extractData(bytes)` | Extract label:value pairs from a filled form |
| `extractCsv(csvText)` | Parse CSV into label:value pairs |
| `mapToForm(extracted, fields)` | Rules-based field mapping with confidence scores |
| `formatForLLM(bytes)` | Format table structure as LLM-readable text |

---

## Building

```bash
# Rust tests
cargo test

# WASM build
wasm-pack build --target web --release
# → pkg/ ready to serve with wizard.html

# CLI
cargo run --bin hwpx-fill -- input.hwpx output.hwpx patches.json
```

Requires: `wasm-pack`, Rust stable, wasm32-unknown-unknown target.

---

## License

MIT — see [LICENSE](LICENSE)

Inspired by [openhwp](https://github.com/openhwp/openhwp) (MIT) and [kordoc](https://github.com/harrymyc/kordoc).
