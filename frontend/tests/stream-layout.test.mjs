import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync("frontend/public/index.html", "utf8");
const css = readFileSync("frontend/public/css/stream.css", "utf8");
const js = readFileSync("frontend/public/js/stream-client.js", "utf8");

assert.match(
  html,
  /assets\/input-overlay\/wasd-extended-numeric\.png/,
  "推流页面应引用 input-overlay 键盘贴图",
);
assert.match(
  html,
  /assets\/input-overlay\/mouse\.png/,
  "推流页面应引用 input-overlay 鼠标贴图",
);

for (const keycode of [49, 50, 51, 52, 53]) {
  assert.match(html, new RegExp(`data-keycodes="${keycode}`), `缺少数字键 ${keycode}`);
}

assert.doesNotMatch(html, /aria-label="Alt"/, "推流键盘不应再显示 Alt");
assert.match(html, /data-keycodes="20"[^>]*aria-label="CapsLock"/, "左侧应包含 CapsLock");
assert.match(
  html,
  /data-row="1"[\s\S]*aria-label="Tab"[\s\S]*data-row="2"[\s\S]*aria-label="CapsLock"[\s\S]*data-row="3"[\s\S]*aria-label="Shift"[\s\S]*data-row="4"[\s\S]*aria-label="Ctrl"/,
  "左侧纵向键应按 Tab、CapsLock、Shift、Ctrl 排列",
);

assert.match(
  html,
  /<section class="mouse[^"]*"[\s\S]*data-direction[\s\S]*<\/section>/,
  "鼠标方向标记应位于鼠标容器内部",
);
assert.doesNotMatch(html, /class="mouse-meta"/, "方向标记不应作为鼠标外部信息块展示");

assert.match(css, /\.overlay-composition/, "推流界面应使用紧凑组合布局");
assert.match(css, /background-image:\s*var\(--sprite\)/, "按键和鼠标应通过预设 sprite 渲染");
assert.match(css, /--viewport-width:\s*660px/, "根画布宽度应覆盖键盘和鼠标组合");
assert.match(css, /overflow:\s*visible/, "推流根节点不应裁切按键描边");

assert.match(js, /directionElement\.dataset\.direction/, "方向变化应反映为鼠标内部状态");
assert.match(js, /wheelElement\.dataset\.wheel/, "滚轮变化应反映为鼠标内部状态");
