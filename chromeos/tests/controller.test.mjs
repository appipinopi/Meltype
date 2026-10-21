// SPDX-License-Identifier: GPL-3.0-or-later
import {test} from "node:test";
import assert from "node:assert/strict";
import {ImeController} from "../controller.mjs";
import {translateKey} from "../keys.mjs";
function fixture(loadOverride) {
  const calls = [], errors = [], saved = [];
  const core = {reset() {}, focusIfNeeded() {}, snapshot: () => "snapshot",
    key: () => JSON.stringify({consumed: true, commits: [], view: {text: "😀か", converting: true,
      candidates: ["😀か", "蚊"], notes: [null, "注釈"], selectedIndex: 1}})};
  const ime = Object.fromEntries(["commitText", "setComposition", "clearComposition", "setCandidates",
    "setCandidateWindowProperties", "setCursorPosition"].map(name => [name, async args => {calls.push([name, args]); return true;}]));
  ime.keyEventHandled = (id, handled) => calls.push(["ack", id, handled]);
  const controller = new ImeController({ime, load: loadOverride ?? (async () => core), persist: async s => saved.push(s), report: e => errors.push(e), now: () => 100});
  controller.activate("meltype-rust"); controller.focus({contextID: 7, type: "text", shouldDoLearning: true});
  return {controller, core, ime, calls, errors, saved};
}
const event = {type: "keydown", key: "a", code: "KeyA"};
test("named/OEM/JIS keys and modifiers map to Core ABI", () => {
  assert.deepEqual(translateKey({...event, shiftKey: true, ctrlKey: true}), {vk: 65, character: 97, modifiers: 3});
  assert.equal(translateKey({key: ";", code: "Semicolon"}).vk, 186);
  assert.equal(translateKey({key: "￥", code: "IntlYen"}).vk, 220);
  assert.equal(translateKey({key: "F10", code: "F10"}).vk, 121);
  assert.equal(translateKey({key: "AudioVolumeUp", code: "AudioVolumeUp"}), null);
});
test("async ack happens once, candidate IDs/UTF16 cursor are correct, keyup is paired", async () => {
  const f = fixture();
  assert.equal(f.controller.key("meltype-rust", event, "one"), undefined);
  await f.controller.queue;
  assert.equal(f.calls.find(c => c[0] === "setComposition")[1].cursor, 3);
  assert.equal(f.calls.find(c => c[0] === "setCursorPosition")[1].candidateID, 1);
  assert.deepEqual(f.calls.filter(c => c[0] === "ack"), [["ack", "one", true]]);
  f.controller.key("meltype-rust", {...event, type: "keyup"}, "two"); await f.controller.queue;
  assert.deepEqual(f.calls.at(-1), ["ack", "two", true]);
});
test("focus change during WASM startup never sends old keys to the new field", async () => {
  let resolve;
  const pending = new Promise(r => {resolve = r;});
  const f = fixture(() => pending);
  f.controller.key("meltype-rust", event, "old");
  await Promise.resolve();
  f.controller.focus({contextID: 8, type: "text"}); resolve(f.core);
  await f.controller.queue;
  assert.deepEqual(f.calls, [["ack", "old", false]]);
});
test("passwords, unknown fields, other engines and injected extension events pass through", () => {
  const f = fixture();
  assert.equal(f.controller.key("other", event, "a"), false);
  assert.equal(f.controller.key("meltype-rust", {...event, extensionId: "foreign"}, "b"), false);
  for (const type of ["password", "null", "number", "tel"]) {
    f.controller.focus({contextID: 7, type});
    assert.equal(f.controller.key("meltype-rust", event, "c"), false);
  }
  assert.equal(f.calls.length, 0);
});
test("failed API and rejected startup still acknowledge the key exactly once", async () => {
  const f = fixture(); f.ime.setComposition = async () => false;
  f.controller.key("meltype-rust", event, "failure"); await f.controller.queue;
  assert.equal(f.errors.length, 1);
  assert.deepEqual(f.calls.filter(c => c[0] === "ack"), [["ack", "failure", true]]);
  const failed = fixture(async () => {throw new Error("corrupt snapshot");});
  failed.controller.key("meltype-rust", event, "startup"); await failed.controller.queue;
  assert.deepEqual(failed.calls, [["ack", "startup", false]]);
});
test("commits are persisted except for contexts that forbid learning", async () => {
  const f = fixture();
  f.core.key = () => JSON.stringify({consumed: true, commits: [{text: "箸", deleteBefore: 0}], view: null});
  f.controller.key("meltype-rust", event, "saved"); await f.controller.queue;
  assert.equal(f.saved.length, 1);
  f.controller.focus({contextID: 7, type: "text", shouldDoLearning: false});
  f.controller.key("meltype-rust", event, "private"); await f.controller.queue;
  assert.equal(f.saved.length, 1);
});
