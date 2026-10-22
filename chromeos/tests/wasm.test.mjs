// SPDX-License-Identifier: GPL-3.0-or-later
import {test} from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {initSync, BrowserSession} from "../pkg/meltype_browser.js";
initSync({module: readFileSync(new URL("../pkg/meltype_browser_bg.wasm", import.meta.url))});
const key = (s, vk, c = 0, mods = 0) => JSON.parse(s.key(vk, typeof c === "string" ? c.codePointAt(0) : c, mods, 100));
const type = (s, text) => { for (const c of text) key(s, c.toUpperCase().charCodeAt(0), c); };

test("website code mode passes source through and permits Japanese in comments and strings", () => {
  const s = new BrowserSession("");
  assert.equal(s.set_code_mode(true), true);
  for (const c of "const answer = 42;") {
    const result = key(s, c.toUpperCase().charCodeAt(0), c);
    assert.equal(result.consumed, false, c);
    assert.deepEqual(result.commits, []);
    assert.equal(result.view, null);
  }
  for (const before of ["// ", 'const message = "']) {
    s.focus(false); s.context(before, "");
    for (const c of "nihongo") assert.equal(key(s, c.toUpperCase().charCodeAt(0), c).consumed, true);
    assert.equal(JSON.parse(s.view()).view.text, "にほんご");
    assert.equal(key(s, 13).commits[0].text, "にほんご");
  }
  s.focus(false);
  assert.equal(s.set_code_mode(false), true);
  assert.equal(key(s, 65, "a").view.text, "あ");
  s.free();
});

test("website mode changes wait for composition and are never saved in the Core snapshot", () => {
  const s = new BrowserSession("");
  type(s, "nihongo");
  const before = s.view();
  assert.equal(s.set_code_mode(true), false);
  assert.equal(s.view(), before);
  assert.equal(key(s, 13).commits[0].text, "にほんご");
  assert.equal(s.set_code_mode(true), true);
  assert.equal(key(s, 65, "a").consumed, false);
  const restored = new BrowserSession(s.snapshot());
  assert.equal(key(restored, 65, "a").view.text, "あ");
  s.free(); restored.free();
});
test("real WASM shares composition, FST candidates, selection and durable learning", () => {
  const s = new BrowserSession("");
  type(s, "hashi");
  const view = key(s, 32, " ").view;
  assert(view.candidates.includes("橋"));
  const index = view.candidates.indexOf("箸");
  assert(index >= 0);
  assert.equal(JSON.parse(s.select(index)).commits[0].text, "箸");
  const restored = new BrowserSession(s.snapshot());
  type(restored, "hashi");
  assert.equal(key(restored, 32, " ").view.text, "箸");
  restored.clear_learning();
  assert.deepEqual(JSON.parse(restored.snapshot()).state.preferences.selected, {});
  s.free(); restored.free();
});
test("uncommitted text is not saved; reset and no-learning focus preserve privacy", () => {
  const s = new BrowserSession("");
  s.focus(false);
  type(s, "hashi");
  const snapshot = s.snapshot();
  assert(!snapshot.includes('"raw"'));
  const view = key(s, 32, " ").view;
  s.select(view.candidates.indexOf("箸"));
  assert.deepEqual(JSON.parse(s.snapshot()).state.preferences.selected, {});
  type(s, "nihongo"); s.reset();
  assert.equal(JSON.parse(s.view()).view, null);
  s.free();
});
test("configuration migration and unknown fields survive a WASM round trip", () => {
  const s = new BrowserSession("");
  s.configure(JSON.stringify({InputStyle: "Both", Mode: "AutoSwitch", futureField: {keep: true}}));
  const settings = JSON.parse(s.snapshot()).settings;
  assert.equal(settings.InputStyle, "Romaji");
  assert.notEqual(settings.Mode, "AutoSwitch");
  assert.deepEqual(settings.futureField, {keep: true});
  assert.throws(() => s.configure("{broken"));
  assert.throws(() => s.configure(JSON.stringify({CursorIntoBrackets: true})));
  assert.throws(() => s.key(65, 97, 0, NaN));
  assert.throws(() => new BrowserSession("{broken"));
  s.free();
});
test("F6/F7 and Enter use the actual shared Core", () => {
  const s = new BrowserSession("");
  type(s, "nihongo");
  assert.equal(key(s, 118).view.text, "ニホンゴ");
  assert.equal(key(s, 117).view.text, "にほんご");
  assert.equal(key(s, 13).commits[0].text, "にほんご");
  s.free();
});

test("real WASM exposes learned kanji predictions for browser candidate clicks", () => {
  const phrase = "橋の近くで待っています";
  const s = new BrowserSession(JSON.stringify({state: {phrases: [{
    reading: "はしのちかくでまっています", text: phrase, count: 3, used: Math.floor(Date.now() / 1000)
  }]}}));
  s.focus(false);
  type(s, "hashi");
  const view = JSON.parse(s.view()).view;
  const index = view.predictions.indexOf(phrase);
  assert(index >= 0);
  assert.equal(JSON.parse(s.select_prediction(index)).commits[0].text, phrase);
  assert.deepEqual(JSON.parse(s.snapshot()).state.preferences.selected, {});
  s.free();
});
