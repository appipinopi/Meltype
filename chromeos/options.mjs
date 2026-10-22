// SPDX-License-Identifier: GPL-3.0-or-later
const status = document.querySelector("#status"), settings = document.querySelector("#settings");
async function request(message) {
  const reply = await chrome.runtime.sendMessage(message);
  if (reply?.error) throw new Error(reply.error);
  return reply;
}
async function show(work) {
  try { await work(); } catch (error) { status.textContent = String(error); }
}
async function reload() {
  const state = await request({op: "status"});
  status.textContent = state.diagnostic;
  if (!state.available) {
    document.querySelectorAll("button, textarea").forEach(element => { element.disabled = true; });
    return;
  }
  const {snapshot} = await request({op: "snapshot"});
  settings.value = JSON.stringify(JSON.parse(snapshot).settings, null, 2);
}
document.querySelector("#reload").addEventListener("click", () => show(reload));
document.querySelector("#settings-form").addEventListener("submit", event => {
  event.preventDefault();
  show(async () => {
    const value = JSON.parse(settings.value);
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("設定はJSONオブジェクトで入力してください。");
    await request({op: "configure", settings: JSON.stringify(value)});
    status.textContent = "設定を保存しました。";
  });
});
document.querySelector("#clear").addEventListener("click", () => show(async () => {
  if (!confirm("学習履歴を削除します。ユーザー辞書と設定は保持します。")) return;
  await request({op: "clear_learning"});
  status.textContent = "学習を削除しました。";
}));
document.querySelector("#export").addEventListener("click", () => show(async () => {
  const {snapshot} = await request({op: "snapshot"});
  const url = URL.createObjectURL(new Blob([snapshot], {type: "application/json"}));
  const link = document.createElement("a");
  link.href = url; link.download = "meltype-chromeos-snapshot.json"; link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  status.textContent = "書き出しには個人の辞書・学習を含みます。";
}));
show(reload);
