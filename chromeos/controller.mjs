// SPDX-License-Identifier: GPL-3.0-or-later
// Chrome callbacks are registered before asynchronous WASM/storage startup.
import {translateKey} from "./keys.mjs";
const editable = new Set(["text", "search", "url", "email"]);
export class ImeController {
  constructor({ime, load, persist, report, now = () => performance.now(), incognito = false}) {
    Object.assign(this, {ime, load, persist, report, now, incognito});
    this.engine = null; this.context = null; this.generation = 0;
    this.queue = Promise.resolve(); this.core = null; this.ownedKeys = new Set();
  }
  ready() {
    if (!this.loading) this.loading = this.load().then(core => (this.core = core));
    return this.loading;
  }
  enqueue(work) {
    const task = this.queue.then(work);
    this.queue = task.catch(error => this.report(String(error)));
    return task;
  }
  invalidate() {
    this.generation++; this.ownedKeys.clear();
    if (this.core) this.core.reset();
  }
  activate(engine) { this.invalidate(); this.engine = engine; this.context = null; }
  deactivate(engine) {
    if (this.engine !== engine) return;
    this.invalidate(); this.engine = null; this.context = null;
  }
  focus(context) {
    this.invalidate();
    this.context = editable.has(context.type) ? {...context} : null;
  }
  blur(id) {
    if (this.context?.contextID !== id) return;
    this.invalidate(); this.context = null;
  }
  valid(engine, id, generation) {
    return this.engine === engine && this.context?.contextID === id && this.generation === generation;
  }
  learning() { return !this.incognito && this.context?.shouldDoLearning !== false; }
  async call(name, parameters) {
    const result = await this.ime[name](parameters);
    if (result === false) throw new Error(`${name} rejected by ChromeOS`);
  }
  async render(result, engine, id, generation) {
    const valid = () => this.valid(engine, id, generation);
    if (result.commits.some(edit => edit.deleteBefore || edit.delete_before || edit.caretBack || edit.caret_back)) {
      throw new Error("This ChromeOS host does not support destructive edits or caret moves");
    }
    for (const edit of result.commits) {
      if (!valid()) return;
      if (edit.text) await this.call("commitText", {contextID: id, text: edit.text});
    }
    if (!valid()) return;
    const view = result.view;
    if (view?.text) {
      await this.call("setComposition", {contextID: id, text: view.text, cursor: view.text.length,
        segments: [{start: 0, end: view.text.length, style: "underline"}]});
    } else {
      await this.call("clearComposition", {contextID: id});
    }
    if (!valid()) return;
    const candidates = view?.converting ? view.candidates : [];
    await this.call("setCandidates", {contextID: id, candidates: candidates.map((candidate, index) => ({
      candidate, id: index, label: index < 9 ? String(index + 1) : "", annotation: view.notes?.[index] ?? ""}))});
    if (!valid()) return;
    await this.call("setCandidateWindowProperties", {engineID: engine, properties: {
      visible: candidates.length > 0, vertical: true, cursorVisible: true, pageSize: 9,
      auxiliaryText: view?.hint ?? "", auxiliaryTextVisible: Boolean(view?.hint), windowPosition: "composition"}});
    if (valid() && candidates.length) await this.call("setCursorPosition", {
      contextID: id, candidateID: view.selectedIndex});
  }
  key(engine, event, requestId) {
    const mapped = translateKey(event);
    const id = this.context?.contextID, generation = this.generation;
    if (!mapped || !this.valid(engine, id, generation) || id === undefined || event.extensionId || event.metaKey) return false;
    if (event.type !== "keydown" && event.type !== "keyup") return false;
    if (typeof requestId !== "string" || !requestId) return false;
    this.enqueue(async () => {
      let handled = false;
      try {
        const core = await this.ready();
        if (!this.valid(engine, id, generation)) return;
        if (event.type === "keyup") {
          handled = this.ownedKeys.delete(event.code);
          return;
        }
        core.focusIfNeeded?.(generation, this.learning());
        const result = JSON.parse(core.key(mapped.vk, mapped.character, mapped.modifiers, this.now()));
        handled = result.consumed;
        if (handled) this.ownedKeys.add(event.code);
        await this.render(result, engine, id, generation);
        if (this.valid(engine, id, generation) && this.learning() && result.commits.length) {
          await this.persist(core.snapshot());
        }
      } catch (error) { this.report(String(error)); }
      finally { this.ime.keyEventHandled(requestId, handled); }
    });
    return undefined;
  }
  candidate(engine, candidateID, button) {
    if (button !== "left" || !Number.isInteger(candidateID) || candidateID < 0) return;
    const id = this.context?.contextID, generation = this.generation;
    if (id === undefined || !this.valid(engine, id, generation)) return;
    this.enqueue(async () => {
      const core = await this.ready();
      if (!this.valid(engine, id, generation)) return;
      const view = JSON.parse(core.view()).view;
      if (!view?.converting || candidateID >= view.candidates.length) return;
      await this.render(JSON.parse(core.select(candidateID)), engine, id, generation);
      if (this.valid(engine, id, generation) && this.learning()) await this.persist(core.snapshot());
    });
  }
}
