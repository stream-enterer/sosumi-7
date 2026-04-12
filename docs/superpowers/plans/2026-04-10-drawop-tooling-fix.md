# DrawOp Diff Tooling Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the DrawOp diff tooling so C++ and Rust produce matching op sequences for 1:1 parameter comparison.

**Architecture:** Three independent bug fixes: (1) add depth tracking to 14 C++ compound methods so only top-level ops are logged, (2) escape newlines in text fields on both C++ and Rust sides, (3) replace positional matching in the diff script with LCS-based alignment. Validated end-to-end on widget_button_normal.

**Tech Stack:** C++ (emPainter.cpp in Eagle Mode 0.96.4), Rust (draw_op_dump.rs), Python (diff_draw_ops.py)

---

### Task 1: Add C++ text escaping helper function

**Files:**
- Modify: `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp:26-40`

This helper will be used by the PaintText and PaintTextBoxed logging blocks to properly escape text for JSON.

- [ ] **Step 1: Add helper function after global variables**

After line 40 (after the `fprint_hex_f64` helper), add:

```cpp
// Helper: write a JSON-escaped string (escapes \, ", \n, \r, \t)
static void fprint_json_string(FILE* f, const char* key, const char* val) {
	fprintf(f, "\"%s\":\"", key);
	if (val) {
		for (const char* p = val; *p; ++p) {
			switch (*p) {
				case '\\': fputs("\\\\", f); break;
				case '"':  fputs("\\\"", f); break;
				case '\n': fputs("\\n", f); break;
				case '\r': fputs("\\r", f); break;
				case '\t': fputs("\\t", f); break;
				default:   fputc(*p, f); break;
			}
		}
	}
	fputc('"', f);
}
```

- [ ] **Step 2: Update PaintText logging block to use helper**

At line 2242, change the logging block from using `%s` for text to using the helper. Replace:

```cpp
	if (g_draw_op_log && g_draw_op_depth == 0) {
		fprintf(g_draw_op_log,
			"{\"seq\":%d,\"op\":\"PaintText\",\"x\":%.17g,\"y\":%.17g,\"text\":\"%s\","
			"\"char_height\":%.17g,\"width_scale\":%.17g,"
			"\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
			g_draw_op_seq++, x, y, text ? text : "", charHeight, widthScale,
			color.GetRed(), color.GetGreen(), color.GetBlue(), color.GetAlpha(),
			canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
```

With:

```cpp
	if (g_draw_op_log && g_draw_op_depth == 0) {
		fprintf(g_draw_op_log,
			"{\"seq\":%d,\"op\":\"PaintText\",\"x\":%.17g,\"y\":%.17g,",
			g_draw_op_seq++, x, y);
		fprint_json_string(g_draw_op_log, "text", text ? text : "");
		fprintf(g_draw_op_log,
			",\"char_height\":%.17g,\"width_scale\":%.17g,"
			"\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
			charHeight, widthScale,
			color.GetRed(), color.GetGreen(), color.GetBlue(), color.GetAlpha(),
			canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
```

- [ ] **Step 3: Update PaintTextBoxed logging block to use helper**

At line 2339, change the logging block similarly. Replace:

```cpp
	if (g_draw_op_log && g_draw_op_depth == 0) {
		fprintf(g_draw_op_log,
			"{\"seq\":%d,\"op\":\"PaintTextBoxed\",\"x\":%.17g,\"y\":%.17g,\"w\":%.17g,\"h\":%.17g,"
			"\"text\":\"%s\",\"max_char_height\":%.17g,"
			"\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
			g_draw_op_seq++, x, y, w, h, text ? text : "", maxCharHeight,
			color.GetRed(), color.GetGreen(), color.GetBlue(), color.GetAlpha(),
			canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
```

With:

```cpp
	if (g_draw_op_log && g_draw_op_depth == 0) {
		fprintf(g_draw_op_log,
			"{\"seq\":%d,\"op\":\"PaintTextBoxed\",\"x\":%.17g,\"y\":%.17g,\"w\":%.17g,\"h\":%.17g,",
			g_draw_op_seq++, x, y, w, h);
		fprint_json_string(g_draw_op_log, "text", text ? text : "");
		fprintf(g_draw_op_log,
			",\"max_char_height\":%.17g,"
			"\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
			maxCharHeight,
			color.GetRed(), color.GetGreen(), color.GetBlue(), color.GetAlpha(),
			canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
```

- [ ] **Step 4: No commit yet — continue to Task 2**

---

### Task 2: Add depth tracking to 5 methods with existing logging blocks

**Files:**
- Modify: `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp`

For each method: add `g_draw_op_depth++;` right after the logging block's closing brace, add `g_draw_op_depth--;` before every `return` in the method body (after the logging block), and add `g_draw_op_depth--;` before the method's final closing brace.

- [ ] **Step 1: PaintEllipse (line 1088, ends at line 1135)**

After the logging block closing brace (after line ~1112), add:

```cpp
	g_draw_op_depth++;
```

Before each return at lines 1113-1117, change from bare `return;` to `{ g_draw_op_depth--; return; }`:

```cpp
	if (x*ScaleX+OriginX>=ClipX2) { g_draw_op_depth--; return; }
	if ((x+w)*ScaleX+OriginX<=ClipX1) { g_draw_op_depth--; return; }
	if (y*ScaleY+OriginY>=ClipY2) { g_draw_op_depth--; return; }
	if ((y+h)*ScaleY+OriginY<=ClipY1) { g_draw_op_depth--; return; }
	if (w<=0.0 || h<=0.0) { g_draw_op_depth--; return; }
```

Before the final `}` at line 1135, add:

```cpp
	g_draw_op_depth--;
```

- [ ] **Step 2: PaintEllipseSector (line 1138, ends at line 1201)**

After the logging block closing brace (after line ~1166), add `g_draw_op_depth++;`.

Before each return at lines 1167-1179, add `g_draw_op_depth--;`. The line 1172-1173 case calls PaintEllipse then returns — add depth-- before the call:

```cpp
	if (rangeAngle==0.0) { g_draw_op_depth--; return; }
	// ...
	if (rangeAngle>=2*M_PI) { PaintEllipse(x,y,w,h,texture,canvasColor); g_draw_op_depth--; return; }
	if (x*ScaleX+OriginX>=ClipX2) { g_draw_op_depth--; return; }
	if ((x+w)*ScaleX+OriginX<=ClipX1) { g_draw_op_depth--; return; }
	if (y*ScaleY+OriginY>=ClipY2) { g_draw_op_depth--; return; }
	if ((y+h)*ScaleY+OriginY<=ClipY1) { g_draw_op_depth--; return; }
	if (w<=0.0 || h<=0.0) { g_draw_op_depth--; return; }
```

Before the final `}` at line 1201, add `g_draw_op_depth--;`.

- [ ] **Step 3: PaintText (line 2232, ends at line 2325)**

After the logging block closing brace (after line ~2256), add `g_draw_op_depth++;`.

The multi-line return at line 2258-2265 needs wrapping:

```cpp
	if (
		y*ScaleY+OriginY>=ClipY2 ||
		(y+charHeight)*ScaleY+OriginY<=ClipY1 ||
		x>=(cx2=(ClipX2-OriginX)/ScaleX) ||
		ClipX1>=ClipX2 ||
		charHeight*ScaleY<=0.1 ||
		widthScale<=0.0
	) { g_draw_op_depth--; return; }
```

Before the final `}` at line 2325, add `g_draw_op_depth--;`.

- [ ] **Step 4: PaintTextBoxed (line 2328, ends at line 2458)**

After the logging block closing brace (after line ~2354), add `g_draw_op_depth++;`.

Before the return at line 2358:

```cpp
	if (tw<=0.0) { g_draw_op_depth--; return; }
```

Before the final `}` at line 2458, add `g_draw_op_depth--;`.

- [ ] **Step 5: PaintSolidPolyline (line 3467, ends at line 3785)**

After the logging block closing brace (after line ~3513), add `g_draw_op_depth++;`.

Before the return at line 3514:

```cpp
	if (n<=0) { g_draw_op_depth--; return; }
```

Before the final `}` at line 3785, add `g_draw_op_depth--;`.

- [ ] **Step 6: No commit yet — continue to Task 3**

---

### Task 3: Add logging blocks + depth tracking to 9 methods without logging

**Files:**
- Modify: `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp`

For each method: add a logging block (matching PaintRoundRect's pattern), then `g_draw_op_depth++;`, then `g_draw_op_depth--;` before every `return` and before the final `}`.

The logging block pattern is:
```cpp
if (g_draw_op_log && g_draw_op_depth == 0) {
    emColor c = <color_source>;
    fprintf(g_draw_op_log,
        "{\"seq\":%d,\"op\":\"<MethodName>\",<params>,"
        "\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
        g_draw_op_seq++, <param_values>,
        c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha(),
        canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
    <hex fields for geometry params>
    fprintf(g_draw_op_log, "}\n");
    fflush(g_draw_op_log);
}
g_draw_op_depth++;
```

- [ ] **Step 1: PaintBezier (line 968, ends line 1085)**

Add after the opening brace of the method body. Parameters: `n`, `xy[]`, `texture.GetColor()`, `canvasColor`.

```cpp
if (g_draw_op_log && g_draw_op_depth == 0) {
    emColor c = texture.GetColor();
    fprintf(g_draw_op_log,
        "{\"seq\":%d,\"op\":\"PaintBezier\",\"n\":%d,"
        "\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
        g_draw_op_seq++, n,
        c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha(),
        canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
    fprintf(g_draw_op_log, "}\n");
    fflush(g_draw_op_log);
}
g_draw_op_depth++;
```

Returns at lines 980, 994, 996, 998, 1000 — add `g_draw_op_depth--;` before each.
Add `g_draw_op_depth--;` before final `}` at line 1085.

- [ ] **Step 2: PaintLine (line 1278, ends line 1335)**

Parameters: `x1, y1, x2, y2, thickness`, `stroke.Color`, `canvasColor`.

```cpp
if (g_draw_op_log && g_draw_op_depth == 0) {
    emColor c = stroke.Color;
    fprintf(g_draw_op_log,
        "{\"seq\":%d,\"op\":\"PaintLine\",\"x1\":%.17g,\"y1\":%.17g,\"x2\":%.17g,\"y2\":%.17g,"
        "\"thickness\":%.17g,"
        "\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
        g_draw_op_seq++, x1, y1, x2, y2, thickness,
        c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha(),
        canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
    fprint_hex_f64(g_draw_op_log, "x1", x1);
    fprint_hex_f64(g_draw_op_log, "y1", y1);
    fprint_hex_f64(g_draw_op_log, "x2", x2);
    fprint_hex_f64(g_draw_op_log, "y2", y2);
    fprint_hex_f64(g_draw_op_log, "thickness", thickness);
    fprintf(g_draw_op_log, "}\n");
    fflush(g_draw_op_log);
}
g_draw_op_depth++;
```

Returns at lines 1287, 1291, 1292, 1295, 1296, 1300, 1301, 1304, 1305 — add `g_draw_op_depth--;` before each.
Add `g_draw_op_depth--;` before final `}` at line 1335.

- [ ] **Step 3: PaintPolyline (line 1338, ends line 1409)**

Parameters: `n`, `xy[]`, `thickness`, `stroke.Color`, `canvasColor`.

```cpp
if (g_draw_op_log && g_draw_op_depth == 0) {
    emColor c = stroke.Color;
    fprintf(g_draw_op_log,
        "{\"seq\":%d,\"op\":\"PaintPolyline\",\"n\":%d,\"thickness\":%.17g,"
        "\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
        g_draw_op_seq++, n, thickness,
        c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha(),
        canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
    fprint_hex_f64(g_draw_op_log, "thickness", thickness);
    fprintf(g_draw_op_log, "}\n");
    fflush(g_draw_op_log);
}
g_draw_op_depth++;
```

Returns at lines 1348, 1361, 1362, 1363, 1364 — add `g_draw_op_depth--;` before each.
Add `g_draw_op_depth--;` before final `}` at line 1409.

- [ ] **Step 4: PaintBezierLine (line 1412, ends line 1587)**

Parameters: `n`, `xy[]`, `thickness`, `stroke.Color`, `canvasColor`.

```cpp
if (g_draw_op_log && g_draw_op_depth == 0) {
    emColor c = stroke.Color;
    fprintf(g_draw_op_log,
        "{\"seq\":%d,\"op\":\"PaintBezierLine\",\"n\":%d,\"thickness\":%.17g,"
        "\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
        g_draw_op_seq++, n, thickness,
        c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha(),
        canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
    fprint_hex_f64(g_draw_op_log, "thickness", thickness);
    fprintf(g_draw_op_log, "}\n");
    fflush(g_draw_op_log);
}
g_draw_op_depth++;
```

Returns at lines 1426, 1435, 1450, 1452, 1454, 1456 — add `g_draw_op_depth--;` before each.
Add `g_draw_op_depth--;` before final `}` at line 1587.

- [ ] **Step 5: PaintEllipseArc (line 1590, ends line 1684)**

Parameters: `x, y, w, h, startAngle, rangeAngle, thickness`, `stroke.Color`, `canvasColor`.

```cpp
if (g_draw_op_log && g_draw_op_depth == 0) {
    emColor c = stroke.Color;
    fprintf(g_draw_op_log,
        "{\"seq\":%d,\"op\":\"PaintEllipseArc\",\"x\":%.17g,\"y\":%.17g,\"w\":%.17g,\"h\":%.17g,"
        "\"start_angle\":%.17g,\"range_angle\":%.17g,\"thickness\":%.17g,"
        "\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
        g_draw_op_seq++, x, y, w, h, startAngle, rangeAngle, thickness,
        c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha(),
        canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
    fprint_hex_f64(g_draw_op_log, "x", x);
    fprint_hex_f64(g_draw_op_log, "y", y);
    fprint_hex_f64(g_draw_op_log, "w", w);
    fprint_hex_f64(g_draw_op_log, "h", h);
    fprint_hex_f64(g_draw_op_log, "start_angle", startAngle);
    fprint_hex_f64(g_draw_op_log, "range_angle", rangeAngle);
    fprint_hex_f64(g_draw_op_log, "thickness", thickness);
    fprintf(g_draw_op_log, "}\n");
    fflush(g_draw_op_log);
}
g_draw_op_depth++;
```

Returns at lines 1605, 1609, 1611, 1615, 1616, 1617, 1618 — add `g_draw_op_depth--;` before each.
Add `g_draw_op_depth--;` before final `}` at line 1684.

- [ ] **Step 6: PaintRectOutline (line 1687, ends line 1741)**

Parameters: `x, y, w, h, thickness`, `stroke.Color`, `canvasColor`.

```cpp
if (g_draw_op_log && g_draw_op_depth == 0) {
    emColor c = stroke.Color;
    fprintf(g_draw_op_log,
        "{\"seq\":%d,\"op\":\"PaintRectOutline\",\"x\":%.17g,\"y\":%.17g,\"w\":%.17g,\"h\":%.17g,"
        "\"thickness\":%.17g,"
        "\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
        g_draw_op_seq++, x, y, w, h, thickness,
        c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha(),
        canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
    fprint_hex_f64(g_draw_op_log, "x", x);
    fprint_hex_f64(g_draw_op_log, "y", y);
    fprint_hex_f64(g_draw_op_log, "w", w);
    fprint_hex_f64(g_draw_op_log, "h", h);
    fprint_hex_f64(g_draw_op_log, "thickness", thickness);
    fprintf(g_draw_op_log, "}\n");
    fflush(g_draw_op_log);
}
g_draw_op_depth++;
```

Returns at lines 1695, 1700, 1702, 1704, 1706, 1713, 1721, 1732 — add `g_draw_op_depth--;` before each.
Add `g_draw_op_depth--;` before final `}` at line 1741.

- [ ] **Step 7: PaintEllipseOutline (line 1744, ends line 1814)**

Parameters: `x, y, w, h, thickness`, `stroke.Color`, `canvasColor`.

```cpp
if (g_draw_op_log && g_draw_op_depth == 0) {
    emColor c = stroke.Color;
    fprintf(g_draw_op_log,
        "{\"seq\":%d,\"op\":\"PaintEllipseOutline\",\"x\":%.17g,\"y\":%.17g,\"w\":%.17g,\"h\":%.17g,"
        "\"thickness\":%.17g,"
        "\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
        g_draw_op_seq++, x, y, w, h, thickness,
        c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha(),
        canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
    fprint_hex_f64(g_draw_op_log, "x", x);
    fprint_hex_f64(g_draw_op_log, "y", y);
    fprint_hex_f64(g_draw_op_log, "w", w);
    fprint_hex_f64(g_draw_op_log, "h", h);
    fprint_hex_f64(g_draw_op_log, "thickness", thickness);
    fprintf(g_draw_op_log, "}\n");
    fflush(g_draw_op_log);
}
g_draw_op_depth++;
```

Returns at lines 1753, 1758, 1760, 1762, 1764, 1787, 1798 — add `g_draw_op_depth--;` before each.
Add `g_draw_op_depth--;` before final `}` at line 1814.

- [ ] **Step 8: PaintEllipseSectorOutline (line 1817, ends line 1873)**

Parameters: `x, y, w, h, startAngle, rangeAngle, thickness`, `stroke.Color`, `canvasColor`.

```cpp
if (g_draw_op_log && g_draw_op_depth == 0) {
    emColor c = stroke.Color;
    fprintf(g_draw_op_log,
        "{\"seq\":%d,\"op\":\"PaintEllipseSectorOutline\",\"x\":%.17g,\"y\":%.17g,\"w\":%.17g,\"h\":%.17g,"
        "\"start_angle\":%.17g,\"range_angle\":%.17g,\"thickness\":%.17g,"
        "\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
        g_draw_op_seq++, x, y, w, h, startAngle, rangeAngle, thickness,
        c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha(),
        canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
    fprint_hex_f64(g_draw_op_log, "x", x);
    fprint_hex_f64(g_draw_op_log, "y", y);
    fprint_hex_f64(g_draw_op_log, "w", w);
    fprint_hex_f64(g_draw_op_log, "h", h);
    fprint_hex_f64(g_draw_op_log, "start_angle", startAngle);
    fprint_hex_f64(g_draw_op_log, "range_angle", rangeAngle);
    fprint_hex_f64(g_draw_op_log, "thickness", thickness);
    fprintf(g_draw_op_log, "}\n");
    fflush(g_draw_op_log);
}
g_draw_op_depth++;
```

Returns at lines 1831, 1837, 1839, 1843, 1844, 1845, 1846 — add `g_draw_op_depth--;` before each.
Add `g_draw_op_depth--;` before final `}` at line 1873.

- [ ] **Step 9: PaintRoundRectOutline (line 1876, ends line 1988)**

Parameters: `x, y, w, h, rx, ry, thickness`, `stroke.Color`, `canvasColor`.

```cpp
if (g_draw_op_log && g_draw_op_depth == 0) {
    emColor c = stroke.Color;
    fprintf(g_draw_op_log,
        "{\"seq\":%d,\"op\":\"PaintRoundRectOutline\",\"x\":%.17g,\"y\":%.17g,\"w\":%.17g,\"h\":%.17g,"
        "\"rx\":%.17g,\"ry\":%.17g,\"thickness\":%.17g,"
        "\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
        g_draw_op_seq++, x, y, w, h, rx, ry, thickness,
        c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha(),
        canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
    fprint_hex_f64(g_draw_op_log, "x", x);
    fprint_hex_f64(g_draw_op_log, "y", y);
    fprint_hex_f64(g_draw_op_log, "w", w);
    fprint_hex_f64(g_draw_op_log, "h", h);
    fprint_hex_f64(g_draw_op_log, "rx", rx);
    fprint_hex_f64(g_draw_op_log, "ry", ry);
    fprint_hex_f64(g_draw_op_log, "thickness", thickness);
    fprintf(g_draw_op_log, "}\n");
    fflush(g_draw_op_log);
}
g_draw_op_depth++;
```

Returns at lines 1885, 1890, 1892, 1894, 1896, 1904, 1935, 1962 — add `g_draw_op_depth--;` before each.
Add `g_draw_op_depth--;` before final `}` at line 1988.

- [ ] **Step 10: Build C++ to verify compilation**

Run: `cd ~/git/eaglemode-0.96.4 && perl make.pl build continue=yes projects=emCore`
Expected: Build succeeds with no errors.

- [ ] **Step 11: Commit C++ changes**

```bash
cd ~/git/eaglemode-0.96.4
git add src/emCore/emPainter.cpp
git commit -m "fix: add depth tracking to all compound Paint methods for DrawOp logging"
```

---

### Task 4: Fix Rust newline escaping in draw_op_dump.rs

**Files:**
- Modify: `/home/a0/git/eaglemode-rs/crates/eaglemode/tests/golden/draw_op_dump.rs:172,182`

- [ ] **Step 1: Add newline/tab escaping to PaintText text field**

At line 172, change:

```rust
            let text = text.replace('\\', "\\\\").replace('"', "\\\"");
```

To:

```rust
            let text = text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t");
```

- [ ] **Step 2: Add newline/tab escaping to PaintTextBoxed text field**

At line 182, change:

```rust
            let text = text.replace('\\', "\\\\").replace('"', "\\\"");
```

To:

```rust
            let text = text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t");
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: Compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/eaglemode/tests/golden/draw_op_dump.rs
git commit -m "fix: escape newlines in DrawOp text serialization for valid JSONL"
```

---

### Task 5: Replace positional matching with LCS alignment in diff script

**Files:**
- Modify: `/home/a0/git/eaglemode-rs/scripts/diff_draw_ops.py`

- [ ] **Step 1: Replace diff_ops function with LCS-aligned version**

Replace the entire `diff_ops` function (lines 58-109) with:

```python
def lcs_alignment(a_types, b_types):
    """LCS-based alignment of two op type sequences.
    Returns list of (a_idx|None, b_idx|None) pairs."""
    m, n = len(a_types), len(b_types)
    # Build LCS table
    dp = [[0] * (n + 1) for _ in range(m + 1)]
    for i in range(m):
        for j in range(n):
            if a_types[i] == b_types[j]:
                dp[i + 1][j + 1] = dp[i][j] + 1
            else:
                dp[i + 1][j + 1] = max(dp[i][j + 1], dp[i + 1][j])

    # Backtrack to find alignment
    pairs = []
    i, j = m, n
    matched = []
    while i > 0 and j > 0:
        if a_types[i - 1] == b_types[j - 1]:
            matched.append((i - 1, j - 1))
            i -= 1
            j -= 1
        elif dp[i - 1][j] >= dp[i][j - 1]:
            i -= 1
        else:
            j -= 1
    matched.reverse()

    # Build full alignment with unmatched entries
    ai, bi = 0, 0
    for ma, mb in matched:
        while ai < ma:
            pairs.append((ai, None))
            ai += 1
        while bi < mb:
            pairs.append((None, bi))
            bi += 1
        pairs.append((ma, mb))
        ai = ma + 1
        bi = mb + 1
    while ai < m:
        pairs.append((ai, None))
        ai += 1
    while bi < n:
        pairs.append((None, bi))
        bi += 1
    return pairs


def diff_ops(cpp_ops, rust_ops, name):
    divergences = []

    cpp_types = [o.get("op", "?") for o in cpp_ops]
    rust_types = [o.get("op", "?") for o in rust_ops]
    alignment = lcs_alignment(cpp_types, rust_types)

    matched = 0
    structural = 0
    for ci, ri in alignment:
        if ci is None:
            rust = rust_ops[ri]
            divergences.append(
                (f"-/{ri}", rust.get("op", "?"), "op", "(absent)", rust.get("op", "?"), "RUST ONLY")
            )
            structural += 1
            continue
        if ri is None:
            cpp = cpp_ops[ci]
            divergences.append(
                (f"{ci}/-", cpp.get("op", "?"), "op", cpp.get("op", "?"), "(absent)", "C++ ONLY")
            )
            structural += 1
            continue

        cpp = cpp_ops[ci]
        rust = rust_ops[ri]
        matched += 1

        all_keys = (set(cpp.keys()) | set(rust.keys())) - SKIP_KEYS
        for key in sorted(all_keys):
            cv = cpp.get(key)
            rv = rust.get(key)
            if cv is None:
                divergences.append((f"{ci}/{ri}", cpp.get("op", "?"), key, "(missing)", fmt(rv), "RUST EXTRA"))
                continue
            if rv is None:
                divergences.append((f"{ci}/{ri}", cpp.get("op", "?"), key, fmt(cv), "(missing)", "C++ EXTRA"))
                continue
            if isinstance(cv, float) and isinstance(rv, float):
                d = abs(cv - rv)
                if d > FLOAT_TOL:
                    divergences.append((f"{ci}/{ri}", cpp.get("op", "?"), key, fmt(cv), fmt(rv), f"{d:.6e}"))
            elif cv != rv:
                divergences.append((f"{ci}/{ri}", cpp.get("op", "?"), key, fmt(cv), fmt(rv), "MISMATCH"))

    print(f"\n=== {name}: {matched} matched, {structural} structural, {len(divergences)} divergence(s) ===")
    if not divergences:
        print("  IDENTICAL")
        return 0

    print(f"{'seq':>7}  {'op':<28} {'param':<20} {'C++':<24} {'Rust':<24} {'delta'}")
    print(f"{'---':>7}  {'---':<28} {'---':<20} {'---':<24} {'---':<24} {'---'}")
    for seq, op, param, cv, rv, delta in divergences:
        print(f"{seq:>7}  {op:<28} {param:<20} {str(cv):<24} {str(rv):<24} {delta}")

    return len(divergences)
```

- [ ] **Step 2: Simplify the load_ops function**

Replace the `load_ops` function (lines 21-47) with a cleaner version now that text is properly escaped:

```python
def load_ops(path):
    ops = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line or not line.startswith("{"):
                continue
            try:
                ops.append(json.loads(line))
            except json.JSONDecodeError:
                pass  # skip unparseable lines
    return ops
```

- [ ] **Step 3: Verify the script runs without errors**

Run: `python3 scripts/diff_draw_ops.py --help 2>&1 || python3 -c "import scripts.diff_draw_ops" 2>&1 || echo "syntax check:" && python3 -c "exec(open('scripts/diff_draw_ops.py').read())" 2>&1`
Expected: No syntax errors (may fail on missing files, that's OK).

- [ ] **Step 4: Commit**

```bash
git add scripts/diff_draw_ops.py
git commit -m "fix: use LCS alignment in DrawOp diff script for structural differences"
```

---

### Task 6: Rebuild C++ generator and validate end-to-end

**Files:**
- No new files — validation only

- [ ] **Step 1: Rebuild emCore with depth tracking changes**

Run: `cd ~/git/eaglemode-0.96.4 && perl make.pl build continue=yes projects=emCore`
Expected: Build succeeds with no errors.

- [ ] **Step 2: Rebuild C++ generator**

Run: `make -C /home/a0/git/eaglemode-rs/crates/eaglemode/tests/golden/gen clean && make -C /home/a0/git/eaglemode-rs/crates/eaglemode/tests/golden/gen`
Expected: Build succeeds, linking against the updated emCore.

- [ ] **Step 3: Regenerate C++ ops**

Run: `make -C /home/a0/git/eaglemode-rs/crates/eaglemode/tests/golden/gen run`
Expected: New .cpp_ops.jsonl files in `crates/eaglemode/target/golden-divergence/`.

- [ ] **Step 4: Generate Rust ops for widget_button_normal**

Run: `cd /home/a0/git/eaglemode-rs && DUMP_DRAW_OPS=1 cargo test --test golden widget_button_normal -- --test-threads=1`
Expected: Test runs and produces `crates/eaglemode/target/golden-divergence/widget_button_normal.rust_ops.jsonl`.

- [ ] **Step 5: Run diff on widget_button_normal**

Run: `python3 scripts/diff_draw_ops.py widget_button_normal`
Expected: 7 matched ops, 0 structural diffs. Divergences should be only ULP-level geometry differences (1e-15 to 1e-12 range), not TYPE MISMATCH or count mismatch.

- [ ] **Step 6: Verify JSONL validity**

Run: `python3 -c "import json; [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/widget_button_normal.cpp_ops.jsonl')]; print('C++ JSONL valid')"` and same for `.rust_ops.jsonl`.
Expected: Both files parse as valid JSONL with no errors.

- [ ] **Step 7: Spot-check a test with text content**

Run: `DUMP_DRAW_OPS=1 cargo test --test golden testpanel_root -- --test-threads=1 && python3 scripts/diff_draw_ops.py testpanel_root`
Expected: Diff produces useful output — matched ops with parameter comparisons, no JSON parse errors from multi-line text. May have structural diffs, but they should be clearly labeled.

- [ ] **Step 8: Commit any remaining changes if needed**

If validation exposed issues that required fixes, commit those fixes.
