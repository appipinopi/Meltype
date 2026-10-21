// SPDX-License-Identifier: GPL-3.0-or-later
import {test} from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
function event() {
  return {listeners: [], addListener(callback) {this.listeners.push(callback);}};
}
function baseChrome() {
  return {runtime: {id: "test-id", getURL: path => `chrome-extension://test-id/${path}`,
    onMessage: event(), openOptionsPage() {}}, action: {onClicked: event()},
    storage: {local: {get: async () => ({}), set: async () => {}}}};
}
test("real service-worker module exposes an actionable status when IME API is absent", async () => {
  globalThis.chrome = baseChrome();
  await import("../background.mjs?unavailable");
  assert.equal(chrome.action.onClicked.listeners.length, 1);
  const message = chrome.runtime.onMessage.listeners[0];
  assert.equal(message({op: "status"}, {id: "foreign", url: "https://example.com/"}, () => assert.fail()), false);
  const reply = await new Promise(resolve => {
    assert.equal(message({op: "status"}, {id: chrome.runtime.id, url: chrome.runtime.getURL("options.html")}, resolve), true);
  });
  assert.equal(reply.available, false);
  assert(reply.diagnostic.includes("chrome.input.ime"));
});
test("real service-worker listeners register before hydration and route keys into actual WASM", {timeout: 10000}, async () => {
  globalThis.chrome = baseChrome();
  const ime = {};
  for (const name of ["onActivate", "onDeactivated", "onFocus", "onInputContextUpdate", "onBlur",
    "onReset", "onKeyEvent", "onCandidateClicked", "onSurroundingTextChanged"]) ime[name] = event();
  const calls = [];
  for (const name of ["setComposition", "clearComposition", "commitText", "setCandidates",
    "setCandidateWindowProperties", "setCursorPosition"]) ime[name] = async args => {calls.push([name, args]); return true;};
  chrome.input = {ime};
  let reads = 0;
  chrome.storage.local.get = async () => {reads++; return {};};
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async url => {
    assert.equal(url, chrome.runtime.getURL("pkg/meltype_browser_bg.wasm"));
    return new Response(readFileSync(new URL("../pkg/meltype_browser_bg.wasm", import.meta.url)), {headers: {"Content-Type": "application/wasm"}});
  };
  try {
    await import("../background.mjs?available");
    assert.equal(reads, 0);
    assert.equal(ime.onKeyEvent.listeners.length, 1);
    ime.onActivate.listeners[0]("meltype-rust");
    ime.onFocus.listeners[0]({contextID: 5, type: "text", shouldDoLearning: false});
    const acknowledged = new Promise(resolve => {ime.keyEventHandled = (id, handled) => resolve({id, handled});});
    const result = ime.onKeyEvent.listeners[0]("meltype-rust", {type: "keydown", key: "a", code: "KeyA"}, "first-key");
    assert.equal(result, undefined);
    assert.deepEqual(await acknowledged, {id: "first-key", handled: true});
    assert.equal(reads, 1);
    assert.equal(calls.find(([name]) => name === "setComposition")[1].text, "あ");
  } finally { globalThis.fetch = originalFetch; }
});
