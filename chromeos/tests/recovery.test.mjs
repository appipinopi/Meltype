// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import {test} from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {initSync, BrowserSession} from "../pkg/meltype_browser.js";
import {ImeController} from "../controller.mjs";
initSync({module: readFileSync(new URL("../pkg/meltype_browser_bg.wasm", import.meta.url))});

function fixture() {
  const core = new BrowserSession("");
  core.focusIfNeeded = () => {};
  const calls = [], errors = [], saved = [];
  const ime = Object.fromEntries(["commitText", "setComposition", "clearComposition", "setCandidates",
    "setCandidateWindowProperties", "setCursorPosition"].map(name => [name, async args => {calls.push([name, args]); return true;}]));
  ime.keyEventHandled = (id, handled) => calls.push(["ack", id, handled]);
  const controller = new ImeController({ime, load: async () => core, persist: async s => saved.push(s), report: e => errors.push(e)});
  controller.activate("test"); controller.focus({contextID: 1, type: "text"});
  async function key(key, code = `Key${key.toUpperCase()}`, type = "keydown") {
    controller.key("test", {key, code, type}, "request");
    await controller.queue;
  }
  async function type(text) { for (const c of text) await key(c); }
  return {core, controller, ime, calls, errors, saved, key, type};
}

test("numpad Enter confirms real WASM composition and owns its keyup", async () => {
  const f = fixture();
  try {
    await f.type("nihongo");
    await f.key("Enter", "NumpadEnter");
    assert.equal(f.calls.find(([name]) => name === "commitText")[1].text, "にほんご");
    assert.equal(JSON.parse(f.core.view()).view, null);
    await f.key("Enter", "NumpadEnter", "keyup");
    assert.deepEqual(f.calls.at(-1), ["ack", "request", true]);
  } finally { f.core.free(); }
});

for (const failure of ["false", "rejection"]) {
  test(`failed commit (${failure}) stays visible and Enter retries it exactly once`, async () => {
    const f = fixture();
    try {
      await f.type("nihongo");
      const commit = f.ime.commitText;
      f.ime.commitText = async () => { if (failure === "rejection") throw new Error("unavailable"); return false; };
      await f.key("Enter", "Enter");
      assert.equal(f.controller.pending.remaining[0].text, "にほんご");
      assert.equal(f.calls.filter(([name]) => name === "setComposition").at(-1)[1].text, "にほんご");
      assert.equal(f.saved.length, 0);
      assert.equal(f.errors.length, 1);
      await f.key("Enter", "NumpadEnter");
      assert.equal(f.controller.pending.remaining[0].text, "にほんご");
      assert.deepEqual(f.calls.at(-1), ["ack", "request", true]);
      f.ime.commitText = commit;
      await f.key("Enter", "NumpadEnter");
      assert.equal(f.controller.pending, null);
      assert.equal(f.calls.filter(([name]) => name === "commitText").length, 1);
      assert.equal(f.saved.length, 1);
      assert.deepEqual(f.calls.at(-1), ["ack", "request", true]);
    } finally { f.core.free(); }
  });
}

test("partial batch retries only the rejected suffix before processing the next letter", async () => {
  const f = fixture();
  try {
    await f.controller.ready();
    const accepted = [];
    f.ime.commitText = async ({text}) => {
      if (text === "後半") return false;
      accepted.push(text); return true;
    };
    await assert.rejects(f.controller.render({commits: [{text: "前半"}, {text: "後半"}], view: null},
      "test", 1, f.controller.generation));
    f.ime.commitText = async ({text}) => {accepted.push(text); return true;};
    await f.key("a");
    assert.deepEqual(accepted, ["前半", "後半"]);
    assert.equal(JSON.parse(f.core.view()).view.text, "あ");
  } finally { f.core.free(); }
});

test("failed candidate confirmation retries the chosen text on the next click", async () => {
  const f = fixture();
  try {
    await f.type("hashi"); await f.key(" ", "Space");
    const index = JSON.parse(f.core.view()).view.candidates.indexOf("箸");
    assert(index >= 0);
    const commit = f.ime.commitText;
    f.ime.commitText = async () => false;
    f.controller.candidate("test", index, "left"); await f.controller.queue;
    assert.equal(f.controller.pending.remaining[0].text, "箸");
    f.ime.commitText = commit;
    f.controller.candidate("test", index, "left"); await f.controller.queue;
    assert.equal(f.controller.pending, null);
    assert.equal(f.calls.filter(([name]) => name === "commitText").length, 1);
    assert.equal(f.calls.find(([name]) => name === "commitText")[1].text, "箸");
  } finally { f.core.free(); }
});

test("focus change discards a failed commit instead of inserting it into another field", async () => {
  const f = fixture();
  try {
    await f.type("nihongo");
    const commit = f.ime.commitText;
    f.ime.commitText = async () => false;
    await f.key("Enter", "Enter");
    f.controller.focus({contextID: 2, type: "text"});
    f.ime.commitText = commit;
    await f.key("a");
    assert.equal(f.calls.some(([name]) => name === "commitText"), false);
    assert.equal(JSON.parse(f.core.view()).view.text, "あ");
  } finally { f.core.free(); }
});
