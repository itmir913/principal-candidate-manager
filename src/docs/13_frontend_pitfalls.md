# 13. 프론트엔드 함정 (Frontend Pitfalls)

여기 적힌 것들은 **빌드도 테스트도 오라클도 잡지 못하고, 사람이 화면을 열어야만
드러나는** 종류다. 이 저장소에는 프론트엔드 자동화 테스트가 없고(도입 보류 판단),
`tools/oracle/front_check.mjs`의 소스 가드도 파생 계산만 대조한다. 그래서 한 번 밟은
함정은 문서로 남긴다.

각 항목은 **실제로 겪은 사례**다. 추측으로 적은 규칙은 없다.

---

## 1. 표 행의 배경색은 `<tr>`이 아니라 `<td>`에 칠한다

### 증상

담임 [라운드 결과]에서 학과명 편집기를 열었다 닫으면 그 행의 색(추천 확정 초록 ·
미선발 빨강)이 **간헐적으로** 사라졌다. 다시 편집기를 열면 돌아왔다.

### 왜 찾기 어려웠나

DOM도 계산된 스타일도 **내내 정상**이었다.

```
tr inline : background: rgb(220, 252, 231)   ← 초록 그대로
td computed: rgba(0, 0, 0, 0) × 7            ← 덮은 것 없음
```

`MutationObserver`로 감시해도 스타일 변경이 한 번도 찍히지 않았고,
`document.elementFromPoint`로 확인해도 행을 가린 요소가 없었다. **상태는 옳은데
화면만 다른 것** — 즉 값의 문제가 아니라 **페인트의 문제**였다.

판별은 강제 리페인트 한 줄로 끝났다.

```js
row.style.display = 'none'; row.offsetHeight; row.style.display = '';
```

이걸로 색이 돌아오면 리페인트 문제다.

### 원인

**`position: sticky` 셀이 있는 표**에서 **행 높이가 바뀌면**(편집기 열기·닫기)
브라우저가 `<tr>`의 배경을 다시 칠하지 않는 경우가 있다. 같은 표의 학생 구분 행이
한 번도 안 깨진 이유는 그 행이 배경을 **`<td>`에 두었기** 때문이다.

### 규칙

sticky 셀이 있는 표에서 행에 의미색을 줄 때는 `<tr>`에 `background`를 주지 말고,
CSS 변수로 내려 `<td>`가 칠하게 한다.

```html
<tr class="result-row" :style="{ '--row-bg': 추천확정 ? '#dcfce7' : '#fee2e2' }">
```
```css
.result-row td { background-color: var(--row-bg, transparent); }
```

sticky가 없는 표라면 `<tr>` 배경도 지금은 동작한다. 다만 나중에 sticky 헤더를 붙이는
순간 같은 버그가 생기므로, **행 높이가 변하는 표라면 처음부터 `<td>`에 두는 편이 낫다.**

---

## 2. 호버 음영은 의미색을 덮어쓰지 말고 위에 겹친다

### 증상

호버 규칙이 `background-color`를 통째로 바꾸면, 그 순간 행의 의미색(추천 확정 ·
미선발)이 사라진다. 마우스를 올린 행만 상태를 알 수 없게 된다.

### 규칙

`background-color`를 바꾸는 대신 `inset box-shadow`를 얹는다. 배경 위·내용 아래에
칠해지므로 행 색이 밑에 그대로 남는다.

```css
.result-row td { background-color: var(--row-bg, transparent); transition: box-shadow .12s ease; }
.result-row:hover td { box-shadow: inset 0 0 0 999px rgba(15, 23, 42, 0.06); }
```

### 함께 주의할 것 — 인라인 배경 vs `hover:bg-*` 클래스

`<tr>`에 **인라인** `background`가 있으면 Tailwind의 `hover:bg-slate-50`은 **애초에
적용되지 않는다**(인라인이 클래스를 이긴다). 조건부로만 인라인 배경을 주는 표에서는
색이 있는 행만 호버 반응이 없어 **행마다 동작이 달라진다.** 둘을 한 요소에 같이 쓰지
않는다.

---

## 3. 고정 레이아웃 표의 셀에 버튼·입력칸을 넣을 때

### 증상

`table-layout: fixed` 표의 좁은 열에 편집기를 넣었더니 내용이 셀 밖으로 **97px** 밀려
옆 열 위에 겹쳐 그려졌다. 다른 곳에서는 버튼이 눌려 글자가 **세로로 3줄** 쪼개졌다.

갈리는 건 버튼에 `white-space: nowrap`이 있느냐뿐이다 — 있으면 밖으로 밀리고, 없으면
안에서 뭉개진다. **어느 쪽이든 결함이다.**

### 원인

flex 항목의 기본 `min-width`는 `auto`라 콘텐츠보다 작아지지 않는다. 고정 레이아웃이라
열도 늘지 않으므로 남는 폭이 그대로 넘친다.

### 규칙

- 입력칸: `min-width: 0; box-sizing: border-box; width: 100%`
- 버튼: `white-space: nowrap` + `flex: 0 0 auto`
- 묶는 컨테이너: 세로 스택(`flex-direction: column`) + 버튼 줄에 `flex-wrap: wrap`
- 열 너비가 부족하면 `<col>`과 표 `min-width`를 함께 올린다

### 검증 방법

앱을 띄우지 않고 확인할 수 있다. **실제 열 너비와 패딩을 그대로 옮긴 독립 HTML**을
만들어 브라우저에서 좌표를 재면 된다.

```js
const cell = td.getBoundingClientRect();
const need = Math.max(...[...td.querySelectorAll('input,button')].map(e => e.getBoundingClientRect().right));
const overflowPx = need - (cell.right - paddingRight);   // 양수면 넘침
```

---

## 4. `<script setup>`의 미정의 식별자는 빌드가 잡지 못한다

### 증상

세 화면에 같은 처리를 복사해 넣다가 한 곳만 함수 이름을 틀렸다
(`cancelDeptEdit` ↔ `cancelEdit`). 호출이 `try` 밖이라 `ReferenceError`가 그대로
던져졌고, 담임 [라운드 결과] 화면이 **아예 열리지 않았다**("아직 개설된 라운드가
없습니다"만 표시).

### 왜 안 잡혔나

vite는 함수 **본문 안의** 미정의 식별자를 컴파일 타임에 보지 않는다. 이 저장소에는
eslint도 타입 검사도 없고, 백엔드 테스트와 오라클은 `.vue` 런타임을 실행하지 않는다.

### 규칙

같은 처리를 여러 화면에 복사할 때 **이름이 파일마다 다를 수 있다는 것**을 전제하고,
붙여 넣은 뒤 그 파일 안에 정의가 있는지 확인한다. 화면을 실제로 열어보는 것이 유일한
최종 확인이다.

### 훑는 방법

`@vue/compiler-sfc`로 SFC를 컴파일해 `bindingMetadata`를 얻고, 템플릿 렌더 코드의
`_ctx.<name>`(스크립트에 바인딩 없는 참조)과 스크립트의 스코프 밖 식별자를 훑으면
전 파일을 한 번에 검사할 수 있다. 정규식만으로 하면 `defineProps` 같은 컴파일러
매크로와 Options API의 `setup()` **정의**를 호출로 오인하니 걸러야 한다.

---

## 검사 목록 (표를 건드릴 때)

| 확인 | 왜 |
|---|---|
| 이 표에 `position: sticky` 셀이 있는가 | 있으면 행 배경을 `<td>`에 (§1) |
| 행 높이가 바뀌는 상호작용이 있는가 | 리페인트 함정의 방아쇠 (§1) |
| 호버가 `background-color`를 바꾸는가 | 의미색을 덮는다 (§2) |
| 인라인 배경과 `hover:bg-*`를 같이 쓰는가 | 호버가 무력화된다 (§2) |
| 셀 안 버튼·입력칸이 열 너비에 들어가는가 | 밀림·뭉개짐 (§3) |
| 복사해 넣은 함수 이름이 이 파일에 있는가 | 빌드가 안 잡는다 (§4) |
