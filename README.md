# hwpx-filler

브라우저에서 직접 실행되는 HWPX 자동 채움 엔진입니다.  
서버 업로드 없이 WebAssembly로 동작하며, 한글(HWPX) 양식의 분석, 데이터 추출, 필드 매핑, 행 복제, 재패킹까지 한 번에 처리합니다.

**[Live Demo](https://merryai-dev.github.io/hwpx-filler/)**  
**[npm package](https://www.npmjs.com/package/hwpx-filler)**  
**[Rust crate](https://crates.io/crates/hwpx-filler)**

---

## 무엇을 해결하나

공공기관과 기업 제안서 양식은 같은 사람 정보를 여러 서식에 반복 입력하게 만듭니다.

- 양식마다 필드명이 조금씩 다릅니다.
- 경력/참여인력 표는 여러 행을 복제해 채워야 합니다.
- HWPX는 ZIP + XML 구조라서 단순 문자열 치환이나 재압축으로 쉽게 망가집니다.

`hwpx-filler`는 이 문제를 다음 순서로 해결합니다.

1. 채워진 소스 HWPX, CSV, JSON 중 하나에서 데이터를 읽습니다.
2. 빈 대상 HWPX 양식을 분석해 어떤 칸이 입력 대상인지 찾습니다.
3. 라벨 기준으로 소스와 대상 필드를 매핑합니다.
4. 셀 값을 채우고 필요한 행을 복제한 뒤, 원본 HWPX를 보존한 형태로 다시 패킹합니다.

핵심 목표는 세 가지입니다.

- 범용성: 서식이 조금 달라도 최대한 깨지지 않게 분석
- 보존성: 이미지/첨부/스타일이 포함된 원본 HWPX를 가능한 그대로 유지
- 프라이버시: 브라우저에서 직접 처리하고, LLM에는 값이 아닌 라벨만 보내는 구조

---

## 최근 변경 (v0.2)

### 다중 섹션 지원

HWPX 문서의 `Contents/section0.xml`, `section1.xml`, ... 모든 섹션을 자동 순회합니다. 13개 WASM export 전부 다중 섹션을 지원하며, `table_index`는 전체 문서에서 글로벌하게 유니크합니다. wizard.html은 변경 없이 그대로 동작합니다.

### 통합 테스트 스위트

`tests/integration.rs`에 16개 테스트가 추가되었습니다. 실제 HWPX 서식 4개(서식5, 코이카, 테스트, 최악의서식)를 fixture로 포함하며, cross-form 매핑(서식5→코이카 17/32 필드)과 행 클론(10→12행) 등 end-to-end 검증을 수행합니다.

### 채움 결과 HTML 미리보기

`renderToHtml` WASM export가 추가되었습니다. 채워진 HWPX의 테이블 구조를 HTML `<table>`로 렌더링합니다. wizard.html의 Step 4에서 다운로드 전에 결과를 확인할 수 있습니다.

### 에러 한글화

Anthropic API 호출 시 HTTP 상태 코드별 한글 에러 메시지를 반환합니다 (401 키 오류, 429 한도 초과, 500 서버 오류, 네트워크 실패). 섹션 누락 등 WASM 에러도 한글화되었습니다.

### 적응형 테이블 인식 + SQLite WASM 학습

표 구조를 자동 분류하고, 사용자 피드백으로 인식 정확도를 개선합니다. 학습 데이터는 SQLite WASM(sql.js + IndexedDB)에 저장되어 세션 간 지속됩니다.

### 커뮤니티 정책 허브

Cloudflare Worker + D1으로 PII 없는 구조 메타데이터를 공유합니다. 다른 사용자의 학습 결과를 시작 시 자동 동기화합니다.

---

## 기능 상세

초기 수준의 "빈 양식 분석 + 단순 셀 패치"를 넘어서, 현재 Rust 구현에는 아래 기능이 추가되어 있습니다.

### 1. 채워진 HWPX에서 역으로 데이터 추출

`src/extractor.rs`는 채워진 HWPX를 읽어 `{label: value}` 형태로 다시 뽑아냅니다.

- 가로형 패턴: `[라벨][값][라벨][값]`
- 세로형 패턴: 헤더 행 + 아래 데이터 행들
- 결과에는 `raw_label`, `normalized_label`, `key`, `value`, `table_index`, `row`, `col`이 포함됩니다.

즉, 이 프로젝트는 "양식을 채우는 엔진"일 뿐 아니라, 이미 작성된 HWPX를 다시 구조화된 데이터로 바꾸는 추출기 역할도 합니다.

### 2. 경력/참여이력 같은 세로형 테이블 추출

일반 인적사항 표는 가로형이지만, 경력사항/참여이력/참여인력 이력표는 세로 헤더형인 경우가 많습니다.  
이번 구현은 세로형 테이블도 추출 대상으로 다룹니다.

- 헤더 행을 감지
- 같은 열의 데이터 셀을 헤더와 연결
- 여러 행이 있으면 `key_행번호` 형태로 분리

이 기능이 있어야 서식 3-5 같은 경력표를 소스 데이터로 재활용할 수 있습니다.

### 3. CSV/Firebase export 입력 지원

`extractCsv`는 CSV 첫 행을 헤더, 첫 데이터 행을 값으로 해석해 HWPX 추출 결과와 동일한 형태로 변환합니다.

- Firebase export처럼 열 이름이 있는 데이터
- 운영 중인 인사 DB에서 한번 내보낸 CSV
- HWPX 없이도 바로 양식 채움에 쓰고 싶은 경우

이 경로 덕분에 소스가 꼭 HWPX일 필요가 없습니다.

### 4. 행 복제 기반의 경력표 채움

`src/patcher.rs`에는 단순 셀 교체 외에 행 복제 기능이 들어 있습니다.

- 특정 `<hp:tr>` 블록 전체를 복제
- 스타일/테두리/레이아웃 속성 유지
- 텍스트만 비워 새 행처럼 사용
- 복제 후 새 행 주소에 맞춰 값 패치

이 기능이 없으면 경력/참여인력 표는 첫 행만 채우고 끝나거나, 수작업 복제가 필요합니다.

### 5. 규칙 기반 상세 매핑 결과

`map_extracted_to_form_detailed`는 단순 패치 목록만 반환하지 않고, 어떤 방식으로 매칭됐는지도 함께 반환합니다.

- canonical key 일치
- normalized label 일치
- 부분 포함 기반 fuzzy 일치
- unmatched 항목 추적

이 결과는 wizard 미리보기와 디버깅에 직접 쓸 수 있습니다.

### 6. LLM 프라이버시 경계용 WASM API

`src/wasm.rs`에는 단순 분석 API 외에 프라이버시 경계를 위한 함수가 추가되어 있습니다.

- `setApiKey`
- `hasApiKey`
- `clearApiKey`
- `callAnthropic`
- `extractLabelsOnly`
- `applyLabelMappings`

핵심은 "값은 WASM 안에 두고, LLM에는 라벨만 보내는 것"입니다.  
즉, `"김철수"`는 보내지 않고 `"성명"`만 보내서 `"직책"`과 `"직책/직위"`가 같은 의미인지 판단하게 합니다.

### 7. 내용물 타입 분류와 구조 검증

현재 구현은 단순히 텍스트 위치만 찾는 것이 아니라, 추가 안전장치도 갖고 있습니다.

- `ContentType`: `TextOnly`, `HasPicture`, `HasEquation`, `HasFormControl`, `HasDrawing`, `Mixed`, `Unknown`
- `validate_stream`: rowCnt, rowAddr 정합성 확인
- `validate_section`, `validate_roundtrip`: serde 경로 기반 검증

즉, "어디를 채울지"뿐 아니라 "이 셀을 건드려도 안전한지"를 판단할 준비가 되어 있습니다.

### 8. 서식 3-5 대응을 위한 라벨 판정 강화

최근 반영된 수정으로 라벨/데이터 분류가 더 보수적으로 바뀌었습니다.

- `borderFillIDRef` 보정은 이제 `data_count == 0`일 때만 라벨 승격
- 서식 3-5에 자주 등장하는 라벨 키워드 추가
- extractor의 연속 헤더 판정 완화

이 수정으로 모든 셀이 같은 스타일을 쓰는 표에서 데이터 셀이 라벨로 오분류되는 문제가 줄었습니다.

---

## 사용 방법

### 1. 브라우저에서 바로 사용

Live Demo를 열면 설치 없이 바로 쓸 수 있습니다.

1. Anthropic API 키를 입력합니다.
2. 소스 데이터(HWPX 또는 CSV)를 올립니다.
3. 대상 빈 양식을 올립니다.
4. 자동 매핑 결과를 확인하고 다운로드합니다.

브라우저 경로의 특징:

- 별도 서버가 없습니다.
- API 키는 localStorage가 아니라 WASM 메모리에만 보관됩니다.
- 외부로 나가는 데이터는 라벨 기반 매핑 요청뿐입니다.

### 2. Rust 라이브러리로 사용

```toml
[dependencies]
hwpx-filler = "0.1"
```

```rust
use hwpx_filler::{extractor, patcher, stream_analyzer, zipper};

let bytes = std::fs::read("form.hwpx")?;
let text_files = zipper::extract_text_files(&bytes)?;

// 모든 섹션 순회 (다중 섹션 자동 지원)
let mut sections: Vec<(&str, &str)> = text_files.iter()
    .filter(|(n, _)| n.starts_with("Contents/section") && n.ends_with(".xml"))
    .map(|(n, c)| (n.as_str(), c.as_str()))
    .collect();
sections.sort_by_key(|(n, _)| *n);

let mut all_fields = Vec::new();
let mut table_offset = 0;
for (_, xml) in &sections {
    let tables = stream_analyzer::analyze_xml(xml);
    let mut fields = stream_analyzer::extract_fields(&tables);
    for f in &mut fields { f.table_index += table_offset; }
    table_offset += tables.len();
    all_fields.extend(fields);
}

// 셀 패치 적용 (첫 번째 섹션 예시)
let (section_name, section_xml) = &sections[0];
let patches = vec![(0usize, 0u32, 1u32, "김철수".to_string())];
let patched_xml = patcher::patch_cells(section_xml, &patches)?;

let mut modified = std::collections::HashMap::new();
modified.insert(section_name.to_string(), patched_xml);
let output = zipper::patch_hwpx(&bytes, &modified)?;
```

### 3. npm 패키지로 사용

```bash
npm install hwpx-filler
```

```js
import init, {
  analyzeForm,
  fillForm,
  cloneRows,
  extractData,
  extractCsv,
  mapToForm,
} from "hwpx-filler";

await init();

const analyzed = JSON.parse(analyzeForm(templateBytes).json);
const extracted = JSON.parse(extractData(sourceBytes).json);
const mapped = JSON.parse(mapToForm(JSON.stringify(extracted), JSON.stringify(analyzed)).json);
const filled = fillForm(templateBytes, JSON.stringify(mapped.patches));
```

### 4. CLI

현재 CLI는 데모/검증용에 가깝고, 템플릿 분석과 샘플 값 채움을 수행합니다.

```bash
cargo run --bin hwpx-fill -- template.hwpx output.hwpx
```

---

## 프로젝트 구조

```text
hwpx-filler/
├── src/
│   ├── stream_analyzer.rs  # streaming XML 파서, 적응형 인식, 라벨/데이터 분류
│   ├── extractor.rs        # 채워진 HWPX/CSV에서 데이터 추출
│   ├── patcher.rs          # 셀 패치, 행 복제, 스킵 감지
│   ├── llm_format.rs       # 테이블을 LLM 친화 텍스트로 변환
│   ├── zipper.rs           # 원본 ZIP 유지형 패치
│   ├── filler.rs           # 분석/채움/검증 통합 API
│   ├── validate.rs         # 구조 검증
│   ├── model.rs            # serde 모델
│   └── wasm.rs             # 브라우저용 18개 WASM export + 다중 섹션 + 프라이버시
├── tests/
│   ├── integration.rs      # 16개 통합 테스트 (synthetic + real fixtures)
│   └── fixtures/           # 실제 HWPX 서식 (서식5, 코이카, 최악의서식 등)
├── worker/                 # Cloudflare Worker + D1 커뮤니티 정책 허브
├── wizard.html             # 브라우저 UI (4-step wizard + HTML 미리보기)
├── demo.html               # 간단한 데모 페이지
└── examples/
```

---

## 핵심 모듈 설명

### `stream_analyzer.rs`

이 프로젝트의 핵심입니다.  
`quick-xml::Reader`로 XML을 스트리밍하면서 테이블 구조만 뽑아냅니다.

왜 streaming인가:

- HWPX 양식은 태그 구조가 일관되지 않은 경우가 많음
- serde로 전체 스키마를 강하게 묶으면 새 양식에서 쉽게 깨짐
- streaming은 모르는 태그를 무시하고 필요한 구조만 읽을 수 있음

라벨 판정은 두 단계로 이루어집니다.

1. 텍스트 패턴 기반
2. `borderFillIDRef` 스타일 기반 보정

텍스트 패턴 기반에는 다음이 들어갑니다.

- 한국어 필드명 키워드
- `"성 명"`, `"직 책"` 같은 띄어쓴 한 글자 패턴
- `"작성자:"`, `"제출자:"` 같은 콜론 접미 패턴

스타일 기반 보정은 매우 보수적으로 동작합니다.

- 특정 fill style이 확정 라벨 셀에만 쓰였을 때만 추가 승격
- 데이터 셀에 한 번이라도 등장한 style은 승격에 사용하지 않음

### `extractor.rs`

채워진 문서에서 데이터를 다시 뽑아냅니다.

- 가로형 표 추출
- 세로형 표 추출
- 라벨 정규화
- canonical key 생성
- 상세 매핑 결과 생성

즉, "타 문서를 양식으로 옮기는 엔진"에 필요한 역방향 파이프라인을 담당합니다.

### `patcher.rs`

HWPX XML을 안전하게 수정합니다.

- 1차 패스: 목표 셀 텍스트 범위 탐색
- 2차 패스: 해당 범위만 문자열 치환

왜 `quick-xml Writer`를 쓰지 않는가:

- Writer는 속성 순서, 공백, namespace 표현을 바꿀 수 있음
- HWPX 뷰어는 이런 사소한 차이에도 민감할 수 있음
- 필요한 텍스트 범위만 바꾸는 편이 더 안전함

행 복제도 이 모듈이 맡습니다.

### `zipper.rs`

HWPX는 ZIP 컨테이너입니다.  
이 모듈은 원본 ZIP을 기준으로 수정된 XML 엔트리만 교체합니다.

장점:

- 이미지/썸네일/폰트 같은 바이너리 엔트리를 그대로 유지
- 전체 unzip/rezip 과정에서 발생하는 손상 위험 감소

### `filler.rs`

상위 통합 API입니다.

- `analyze`
- `fill`
- `fill_with_rows`
- `validate_patched`

CLI와 WASM 계층이 이 모듈을 통해 공통 흐름을 사용할 수 있게 정리되어 있습니다.

### `validate.rs`

패치 이후 구조가 깨졌는지 검사합니다.

- `rowCnt`와 실제 행 수 비교
- `rowAddr` 역전 여부 확인
- 같은 행 안에서 `rowAddr` 일관성 확인

---

## 브라우저/WASM API

현재 브라우저용 export는 18개입니다. 모든 함수가 다중 섹션을 자동 지원합니다.

| 함수 | 설명 |
|------|------|
| `setApiKey(key)` | API 키를 WASM 메모리에만 저장 |
| `clearApiKey()` | 메모리에서 API 키 제거 |
| `hasApiKey()` | API 키 존재 여부만 확인 |
| `callAnthropic(body)` | Anthropic API를 Rust fetch로 직접 호출 (한글 에러 메시지) |
| `extractLabelsOnly(bytes)` | 값 없이 라벨 목록만 추출 |
| `applyLabelMappings(src, tpl, pairs)` | 라벨 쌍을 이용해 WASM 내부에서 최종 채움 |
| `analyzeForm(bytes)` | 빈 양식 분석 |
| `analyzeFormAdaptive(bytes, policy)` | 적응형 테이블 인식 + trace |
| `inspectTables(bytes, policy)` | 테이블 구조 inspection |
| `fillForm(bytes, patches)` | 셀 패치 적용 |
| `cloneRows(bytes, clones)` | 행 복제 |
| `formatForLLM(bytes)` | LLM 입력용 테이블 포맷 생성 |
| `renderToHtml(bytes)` | 테이블 구조를 HTML로 렌더링 (미리보기) |
| `extractData(bytes)` | 채워진 HWPX에서 데이터 추출 |
| `extractDataAdaptive(bytes, policy)` | 적응형 데이터 추출 + trace |
| `extractCsv(csv)` | CSV를 추출 결과 형태로 변환 |
| `mapToForm(extracted, fields)` | 규칙 기반 매핑 및 상세 결과 반환 |
| `updateRecognitionPolicy(policy, feedback)` | 구조 피드백으로 인식 정책 갱신 |

---

## 설계 포인트

### 1. serde 전체 파싱에만 의존하지 않음

전체 파싱은 enrichment 용도로만 쓰고, 기본 분석은 항상 streaming 경로가 담당합니다.  
즉, 새로운 양식이 들어와도 완전히 실패하기보다 "가능한 만큼 분석"하는 쪽을 택했습니다.

### 2. ZIP을 다시 만드는 대신 원본을 보존

원본 archive를 최대한 유지하는 것이 한글 문서에서는 중요합니다.  
이 프로젝트는 수정된 XML 엔트리만 교체하는 방식으로 손상 위험을 줄입니다.

### 3. LLM은 값 추출기가 아니라 의미 매퍼

LLM이 해야 할 일은 `"직책"`과 `"직책/직위"`가 같은 의미인지 판단하는 것입니다.  
값까지 보내면 프라이버시만 약해지고, 매핑 품질이 좋아지는 것도 아닙니다.

### 4. 행 복제를 일급 기능으로 취급

실무 문서는 단일 값 채움보다 반복 행 채움이 더 중요합니다.  
그래서 행 복제를 보조 기능이 아니라 핵심 기능으로 넣었습니다.

---

## 보안/프라이버시 모델

### 기본 원칙

- 브라우저 내 처리
- 서버 없음
- localStorage에 API 키 저장 안 함
- 값이 아닌 라벨만 LLM에 전달

### WASM을 신뢰 경계로 사용

JavaScript와 WASM 사이를 분리해 민감 정보를 가급적 WASM 안에서만 다루도록 구성했습니다.

- API 키는 `thread_local<Option<String>>`에 저장
- 소스 HWPX에서 뽑은 값 맵도 WASM 내부에서 사용
- JS는 라벨 목록, 라벨 쌍, 최종 결과 바이트만 주고받음

즉, 브라우저 UI는 오케스트레이션만 하고 실제 값 조회와 좌표 계산은 `applyLabelMappings` 안에서 끝냅니다.

### 방어 대상

- 제3자 서버로의 PII 유출
- JS 레벨에서의 API 키 장기 노출

### 방어하지 않는 것

- 악성 브라우저 확장
- OS 수준 키로거
- 완전히 손상된 브라우저 런타임

### 한계

아래는 정직하게 인정하는 한계입니다.

1. 사용자가 처음 입력하는 API 키는 호출 순간 잠깐 JS 문자열로 존재합니다.
2. 소스 파일 바이트 자체는 브라우저 메모리 안에 있으므로 악성 확장에 안전하다고 주장할 수는 없습니다.
3. Anthropic에 전달되는 라벨 정보는 Anthropic 정책의 적용을 받습니다.

---

## 빌드

필수 도구:

- Rust stable
- `wasm32-unknown-unknown` target
- `wasm-pack`

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

테스트:

```bash
cargo test                    # unit 14개 + integration 16개 = 30개
cargo test --test integration # 통합 테스트만
```

브라우저용 WASM 빌드:

```bash
wasm-pack build --target web --release -- --features wasm
python3 -m http.server 8080
```

그 다음 `http://localhost:8080/wizard.html`을 열면 됩니다.

npm publish용 빌드:

```bash
wasm-pack build --target bundler --release -- --features wasm
```

---

## 배포

### GitHub Pages

`master` 브랜치에 push하면 GitHub Actions가 자동으로 배포합니다.

- workflow: `.github/workflows/deploy.yml`
- 빌드: `wasm-pack build --target web --release -- --features wasm`
- 산출물: `wizard.html` + `pkg/`

### npm

버전 태그 `v*`를 push하면 npm publish workflow가 실행됩니다.

- workflow: `.github/workflows/npm-publish.yml`
- 빌드: `wasm-pack build --target bundler --release -- --features wasm`

---

## 지원하는 양식 패턴

| 패턴 | 분석 | 추출 | 채움 | 행 복제 |
|------|------|------|------|---------|
| 인적사항 가로형 표 | ✅ | ✅ | ✅ | - |
| 경력/참여이력 세로형 표 | ✅ | ✅ | ✅ | ✅ |
| 서식 3-5 류 균일 스타일 표 | ✅ | ✅ | ✅ | ✅ |
| CSV/Firebase export 입력 | - | ✅ | ✅ | - |
| 다중 테이블 문서 | ✅ | ✅ | ✅ | 경우에 따라 |
| 다중 섹션 문서 (section0~N) | ✅ | ✅ | ✅ | ✅ |

---

## 알려진 한계

- 병합 셀(colspan/rowspan)은 대표 셀 기준으로만 해석될 수 있습니다.
- 체크박스, 라디오 버튼 같은 폼 컨트롤은 아직 텍스트처럼 완전하게 다루지 않습니다.
- 암호화된 HWPX는 지원하지 않습니다.
- CLI는 아직 범용 소스-대상 매핑 도구라기보다 디버그/데모 성격이 강합니다.
- 빈 `<hp:t></hp:t>` 셀에 연속 패치 시 바이트 오프셋 밀림이 발생할 수 있습니다.

---

## 관련 프로젝트

- [openhwp](https://github.com/openhwp/openhwp)
- [kordoc](https://github.com/harrymyc/kordoc)
- [unhwp](https://github.com/ebkalderon/unhwp)
- [hwp.js](https://github.com/hahnlee/hwp.js)

---

## 라이선스

MIT License. 자세한 내용은 [LICENSE](LICENSE)를 참고하세요.
