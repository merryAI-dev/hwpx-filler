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
Enter your Anthropic API key once (stored in WASM memory — not localStorage, not the server).

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

## Security Architecture

> This section describes the privacy model in detail for security-conscious users and contributors.

### 1. Threat Model

hwpx-filler handles two categories of sensitive data:

| Data | Examples | Risk if leaked |
|------|----------|----------------|
| **PII** | name, phone, birthdate, address, employment history | identity theft, doxxing |
| **API key** | `sk-ant-...` | unauthorized API charges, potential account compromise |

The primary threat we defend against is **data exfiltration to third parties** — specifically, preventing PII and API keys from reaching any server (ours or a third party's) other than Anthropic's API. The secondary threat is **JavaScript-level key exposure**, which is the attack surface of most browser-based API key implementations.

We explicitly do **not** claim to protect against: compromised browser extensions, OS-level keyloggers, a fully compromised browser, or a malicious page in the same origin.

---

### 2. WebAssembly as a Trust Boundary

hwpx-filler uses the WASM module boundary as a security enforcement layer, following the same pattern as [Signal's libsignal](https://github.com/signalapp/libsignal) and privacy-preserving tools like [Local-Sanitizer](https://news.ycombinator.com/item?id=46980613).

The core property: **WASM linear memory is not accessible to JavaScript unless explicitly exported.** A Rust function that takes sensitive input and returns only a non-sensitive output creates an opaque computation that JavaScript cannot inspect.

```
┌─────────────────────────────────────────────────────────────┐
│  JavaScript (untrusted execution context)                   │
│                                                             │
│  Source labels: ["성 명", "직 책", ...]   ──────────────┐   │
│  LLM label pairs: [{src, tgt}, ...]       ──────────────┤   │
│                                           ▼             │   │
│  ┌──────────────────────────────────────────────────┐   │   │
│  │  WASM Module (Rust, trusted)                     │   │   │
│  │                                                  │   │   │
│  │  API key ──► thread_local<Option<String>>         │   │   │
│  │  source bytes ──► extractor::extract_data()      │   │   │
│  │  label→value map  (never exported)               │   │   │
│  │  label→(row,col)  (never exported)               │   │   │
│  │  fetch() with key injected in Rust               │   │   │
│  │                                                  │   │   │
│  └─────────────────────┬────────────────────────────┘   │   │
│                        │                                │   │
│  ◄── filled HWPX bytes (opaque, no intermediate PII) ──┘   │
│  ◄── API response JSON (label pairs only, no key)           │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

### 3. PII Flow Analysis

#### 3.1 Old Architecture (pre-v0.2) — What we fixed

In the original implementation, the LLM mapping function received a fully-rendered table string:

```
Table 0 (2행 × 6열):
  Row 0: [성 명] [김철수] [직 책] [대리] [생년] [1990.03.15]  ← PII
  Row 1: [E-mail] [kim@test.com] [휴대전화] [010-1111-2222]   ← PII
```

This string was passed verbatim to the Anthropic API. **Names, phone numbers, dates, and email addresses were all transmitted to a third-party service.**

#### 3.2 New Architecture (v0.2+) — What is sent to Anthropic

The LLM now receives **only structural information** — label names with no associated values:

```json
// Source document: label names only
["성 명", "직 책", "소 속", "전화번호", "이메일"]

// Target form: labels + empty cell markers (□)
"Table 0:\n  Row 0: [성 명] [□] [소 속] [□] [직책/직위] [□]"
```

The LLM's task is reduced to **semantic label matching only**: understanding that "직책" and "직책/직위" refer to the same concept. It never sees the values.

**Value lookup and coordinate resolution happen entirely inside WASM:**

```
LLM returns:  [{sourceLabel: "성 명", targetLabel: "성 명"}, ...]
                          ↓
WASM applyLabelMappings():
  source_map["성 명"]  →  "김철수"    (from source bytes, never exported)
  target_map["성 명"]  →  (0, 0, 1)   (table 0, row 0, col 1)
  patch_cells(...)     →  filled HWPX bytes
                          ↓
JavaScript receives:   Uint8Array  (complete file, no intermediate PII)
```

#### 3.3 What JavaScript can and cannot see

| Data | JS Access | How |
|------|-----------|-----|
| Source HWPX bytes | ✅ (input) | User uploads, `Uint8Array` passed to WASM |
| Extracted label names | ✅ (output of `extractLabelsOnly`) | For LLM prompt construction |
| Extracted values (PII) | ❌ | Computed inside WASM, never returned |
| Cell coordinates | ❌ | Computed inside WASM `applyLabelMappings` |
| Filled HWPX bytes | ✅ (output) | For browser download |
| LLM label pairs | ✅ (intermediate) | Used to call `applyLabelMappings` |

> **Note on `S.sourceData`:** The rules-based fallback path (`mapToForm`) and the mapping preview UI do hold label–value pairs in JavaScript for display purposes. Values are visible to the user reviewing their own data — this is expected behavior. What we prevent is those values being **transmitted externally** (to LLM or any server).

---

### 4. API Key Handling

#### 4.1 Storage model

| Implementation | Storage | Scope | XSS risk |
|----------------|---------|-------|----------|
| Naive (common) | `localStorage` | Persistent, all JS on origin | High — survives page reload, readable by any injected script |
| Session storage | `sessionStorage` | Tab lifetime | Medium — readable by any same-tab JS |
| **hwpx-filler** | WASM `thread_local` | WASM instance lifetime | **Low** — not accessible to JS after `setApiKey()` call |

After `setApiKey(key)` is called, the key exists only in WASM linear memory. JavaScript holds no reference to it. When the tab is closed or the page is refreshed, the WASM instance is destroyed and the key is gone.

#### 4.2 API call execution

The HTTP request to `api.anthropic.com` is made by Rust using `web-sys` fetch:

```rust
// In Rust (wasm.rs)
pub async fn call_anthropic(body_json: &str) -> Result<String, JsError> {
    let key = API_KEY.with(|k| k.borrow().clone())   // read from thread_local
        .ok_or_else(|| JsError::new("API key not set"))?;

    headers.set("x-api-key", &key)    // key injected here, in Rust
    // JavaScript never receives the headers object before it's consumed by Request
    let request = Request::new_with_str_and_init(url, &opts)?;
    JsFuture::from(window.fetch_with_request(&request)).await
    // JS receives only the response text
}
```

JavaScript passes only the request body (which contains only label names, not PII, not the key). The `x-api-key` header is injected in Rust, inside the WASM boundary.

#### 4.3 What is sent to Anthropic

The complete Anthropic API payload contains:
- `model`, `max_tokens`, `output_config` — model configuration only
- `messages[0].content` — the prompt with label lists and form structure

It does **not** contain: actual field values, user identification, or any PII from the source document.

---

### 5. Limitations and Honest Caveats

1. **Initial key entry.** When the user types their API key into the input field and `setApiKey()` is called, the key briefly exists as a JavaScript string on the call stack. This is unavoidable for any browser-based key entry. An attacker with debugger access or a breakpoint can capture it at this moment.

2. **Source bytes are in JavaScript.** `S.sourceBytes` (the `Uint8Array`) is a JavaScript object. A malicious browser extension or injected script with access to the page's JS context can read it directly, independent of our WASM boundary.

3. **SharedArrayBuffer caveat.** If the page were configured to use `SharedArrayBuffer` for the WASM memory (it is not), JavaScript would have direct access to WASM linear memory. hwpx-filler does not use `SharedArrayBuffer`.

4. **Anthropic's data handling.** Data sent to `api.anthropic.com` is subject to [Anthropic's privacy policy](https://www.anthropic.com/privacy). We minimize what we send (labels only), but we cannot control how Anthropic processes API requests.

5. **Rules-based path.** When no API key is provided, the rules-based fallback path (`mapToForm`) operates entirely in WASM and JavaScript memory without any network calls. No data leaves the browser.

---

### 6. Verification

To independently verify the privacy properties:

```bash
# Inspect the compiled WASM: confirm no outbound fetch calls outside callAnthropic
wasm-objdump -x pkg/hwpx_filler_bg.wasm | grep import

# Review the network tab in browser DevTools:
# - Only one outbound request to api.anthropic.com
# - Request body should contain only label names, not values
# - No requests to any other domain
```

The Rust source for all trust-boundary functions is in [`src/wasm.rs`](src/wasm.rs). The `callAnthropic`, `setApiKey`, `extractLabelsOnly`, and `applyLabelMappings` functions are the complete implementation of the privacy boundary described above.

---

## License

MIT — see [LICENSE](LICENSE)

Inspired by [openhwp](https://github.com/openhwp/openhwp) (MIT) and [kordoc](https://github.com/harrymyc/kordoc).
