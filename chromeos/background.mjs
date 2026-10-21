// SPDX-License-Identifier: GPL-3.0-or-later
import init, {BrowserSession} from "./pkg/meltype_browser.js";
import {ImeController} from "./controller.mjs";
const available = Boolean(chrome.input?.ime?.onKeyEvent);
let diagnostic = available ? "Rust Coreの起動待ち" : "この環境ではchrome.input.imeを利用できません。ChromeOSの対応状況・管理ポリシーを確認してください。";
const report = message => { diagnostic = message; console.warn("Meltype:", message); };
const controller = available ? new ImeController({
  ime: chrome.input.ime, report, incognito: chrome.extension?.inIncognitoContext === true,
  load: async () => {
    const saved = await chrome.storage.local.get("snapshot");
    await init({module_or_path: chrome.runtime.getURL("pkg/meltype_browser_bg.wasm")});
    // Invalid data is preserved. Never silently replace it with defaults.
    const core = new BrowserSession(saved.snapshot ?? "");
    let focusedGeneration = -1;
    core.focusIfNeeded = (generation, learning) => {
      if (focusedGeneration !== generation) { core.focus(learning); focusedGeneration = generation; }
    };
    diagnostic = "Rust Core起動済み。ChromeOSの入力方法にMeltype Rustを追加してください。";
    return core;
  },
  persist: snapshot => chrome.storage.local.set({snapshot}),
}) : null;
if (controller) {
  const ime = chrome.input.ime;
  ime.onActivate.addListener(engine => controller.activate(engine));
  ime.onDeactivated.addListener(engine => controller.deactivate(engine));
  ime.onFocus.addListener(context => controller.focus(context));
  ime.onInputContextUpdate.addListener(context => controller.focus(context));
  ime.onBlur.addListener(id => controller.blur(id));
  ime.onReset.addListener(engine => { if (controller.engine === engine) controller.invalidate(); });
  ime.onKeyEvent.addListener((engine, event, requestId) => controller.key(engine, event, requestId));
  ime.onCandidateClicked.addListener((engine, id, button) => controller.candidate(engine, id, button));
  ime.onSurroundingTextChanged.addListener((engine, info) => {
    const generation = controller.generation, id = controller.context?.contextID;
    controller.enqueue(async () => {
      const core = await controller.ready();
      if (!controller.valid(engine, id, generation) || id === undefined) return;
      if (!Number.isInteger(info.focus) || info.focus !== info.anchor || info.focus < 0 || info.focus > info.text.length) return;
      core.focusIfNeeded(generation, controller.learning());
      core.context(info.text.slice(0, info.focus), info.text.slice(info.focus));
    });
  });
}
chrome.action.onClicked.addListener(() => chrome.runtime.openOptionsPage());
chrome.runtime.onMessage.addListener((message, sender, respond) => {
  if (sender.id !== chrome.runtime.id || sender.url !== chrome.runtime.getURL("options.html")) return false;
  (async () => {
    if (message.op === "status") return {available, diagnostic};
    if (!controller) throw new Error(diagnostic);
    return controller.enqueue(async () => {
      const core = await controller.ready();
      if (message.op === "snapshot") return {snapshot: core.snapshot()};
      if (message.op === "configure") {
        core.configure(message.settings);
        controller.invalidate();
      } else if (message.op === "clear_learning") core.clear_learning();
      else throw new Error("Unknown command");
      await chrome.storage.local.set({snapshot: core.snapshot()});
      return {saved: true};
    });
  })().then(respond, error => respond({error: String(error)}));
  return true;
});
