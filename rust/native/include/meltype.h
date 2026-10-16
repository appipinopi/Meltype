/* SPDX-License-Identifier: GPL-3.0-or-later
 * Copyright (C) 2026 appipinopi */
#ifndef MELTYPE_H
#define MELTYPE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* All strings are NUL-terminated UTF-8. Free results with meltype_free.
 * A session is an opaque token: never dereference it. Serialize calls per
 * session. Callback functions must remain valid until all sessions are gone.
 * Callbacks return malloc-allocated strings; the engine calls free on them. */
typedef char *(*meltype_clauses_callback)(const char *, const char *);
typedef char *(*meltype_candidates_callback)(const char *);
typedef int32_t (*meltype_word_callback)(const char *);
int32_t meltype_init(meltype_clauses_callback, meltype_candidates_callback, meltype_word_callback);
int32_t meltype_init_mozc(const char *, const char *);
void *meltype_create(void);
void *meltype_create_at(const char *directory);
char *meltype_control(void *, const char *request_json);
void meltype_destroy(void *);
char *meltype_handle_key(void *, int32_t vk, int32_t scalar, int32_t modifiers, const char *before, const char *after);
char *meltype_commit(void *);
/* Record the displayed preedit after a host has committed it on focus loss.
 * The host must not apply the returned text again. No backward edit is made. */
char *meltype_commit_preedit(void *);
/* Undo requires a fresh caret snapshot and set_can_delete(1).
 * Hosts MUST compare edit.expect before applying deletion; count is UTF-16. */
char *meltype_undo_commit(void *, const char *before);
/* Verify result.selectionExpect against the current selection before applying
 * view. reading may be NULL; dictionary/history inference can fail safely. */
char *meltype_reconvert_selection(void *, const char *text, const char *reading);
char *meltype_reset(void *);
char *meltype_select_candidate(void *, int32_t index);
char *meltype_select_prediction(void *, int32_t index);
char *meltype_role_key(void *, int32_t vk);
void meltype_set_direct(void *, int32_t);
/* Auto detection additionally requires fresh before text and can_delete(1). */
void meltype_set_direct_detection(void *, int32_t);
void meltype_set_code_input(void *, int32_t);
void meltype_set_private(void *, int32_t);
void meltype_set_can_delete(void *, int32_t);
char *meltype_version(void);
char *meltype_data_directory(void);
char *meltype_report_url(const char *);
void meltype_free(void *);
#ifdef _WIN32
/* Owned TSF COM references, released with IUnknown::Release, NOT meltype_free.
   guid is a pointer to a Windows GUID; unknown/null GUIDs return NULL. */
void *meltype_tsf_create_display_attribute(const void *guid);
void *meltype_tsf_create_display_attribute_enum(void);
/* TSF mode button: COM Release owns the returned interface. Call detach before
   destroying the context or callback code; callbacks run on the creating thread. */
void *meltype_tsf_create_langbar(void *context, int32_t (*read_mode)(void *), void (*toggle)(void *));
void meltype_tsf_langbar_update(void *button);
void meltype_tsf_langbar_detach(void *button);
/* Thread-affine opaque candidate-window token. show takes UTF-8 JSON fields
   candidates/notes/selected/suggestion/meaning/prediction/typed and a Windows RECT.
   Click callbacks may hide/destroy the token. Destroy on its creating thread. */
void *meltype_tsf_candidate_create(void *context, void (*select)(void *, int32_t, int32_t));
int32_t meltype_tsf_candidate_show(void *, const char *json, const void *anchor_rect);
/* TSF core-view projection and placement. Borrowed context/composition, valid
   read cookie and UTF-8 reply view. No COM calls for hidden views/stale tokens. */
int32_t meltype_tsf_candidate_show_tsf(void *token, void *context, void *composition, uint32_t cookie, const char *view);
/* Writes a RECT. Return 1 for TSF text geometry, 0 for caret/cursor fallback,
   -1 for invalid JSON/null output. Never edits the document. */
int32_t meltype_tsf_candidate_anchor(void *context, void *composition, uint32_t cookie, const char *view, void *rect);
void meltype_tsf_candidate_hide(void *);
int32_t meltype_tsf_candidate_visible(void *);
void meltype_tsf_candidate_destroy(void *);
/* COM class factory: GUIDs/out and HRESULT follow the Windows ABI. The shell
   callbacks construct the remaining text service and pin/unpin its DLL. */
int32_t meltype_tsf_get_class_object(const void *clsid, const void *iid, void **out,
    int32_t (*create)(const void *iid, void **out), void (*lock_shell)(int32_t));
/* Explicit regsvr32 operations; mutate COM/profile/category registration.
   path is the TSF shell DLL's UTF-16 path, length excludes a terminator. */
int32_t meltype_tsf_register_server(const uint16_t *path, uint32_t length);
int32_t meltype_tsf_unregister_server(void);
/* Borrowed ITfContext; transfers action_context ownership on every return path.
   destroy is required and called exactly once. Keep callbacks/code alive until
   destroy, including async requests. Use on the creating TSF thread. */
int32_t meltype_tsf_request_edit_session(void *context, uint32_t client, uint32_t flags,
    void *action_context, int32_t (*action)(void *, uint32_t), void (*destroy)(void *));
/* Borrowed TSF COM pointers and edit cookie. UTF-16 callback buffers are borrowed
   only during the callback. Surrounding limits: 4000 before / 20 after. */
int32_t meltype_tsf_read_surrounding(void *context, uint32_t cookie, void *receiver,
    void (*callback)(void *, const uint16_t *, uint32_t, const uint16_t *, uint32_t));
/* Returns 0 unless the range is collapsed and the exact UTF-16 prefix matches.
   Discard the range on failure; never write/delete after a failed check. */
int32_t meltype_tsf_extend_previous(void *range, uint32_t cookie, int32_t count,
    const uint16_t *expected, uint32_t length);
int32_t meltype_tsf_write_range(void *context, void *range, uint32_t cookie,
    const uint16_t *text, uint32_t length, int32_t keep_caret);
/* Start returns an owned ITfComposition. A NULL range uses the current selection.
   All inputs are borrowed COM pointers. Composition views are UTF-8 JSON with
   text/converting/clauses/selectedClause. End retains references across callbacks
   and does not consume the caller's owned COM references. */
void *meltype_tsf_start_composition(void *context, void *sink, void *range, uint32_t cookie);
int32_t meltype_tsf_update_composition(void *context, void *composition, uint32_t cookie,
    const char *view, uint32_t input_atom, uint32_t converted_atom, uint32_t target_atom);
int32_t meltype_tsf_end_composition(void *context, void *composition, uint32_t cookie);
void meltype_tsf_set_attribute(void *context, void *range, uint32_t cookie, uint32_t atom);
/* Synchronous batch; owner/callbacks must remain valid until return. Current
   returns a borrowed composition. Write borrows the range and UTF-16 text.
   Return 0 rejects the remaining batch; caller must reset the core session. */
int32_t meltype_tsf_apply_commits(void *context, uint32_t cookie, const char *commits,
    void *owner, void *(*current)(void *), int32_t (*write)(void *, void *, const uint16_t *, uint32_t));
/* Thread-affine opaque resume snapshot: capture only eligible unconverted input.
   Try consumes on success/failure, returns owned ITfComposition, and never inserts
   text. Destroy unused tokens on the creating thread. Matches compares context. */
void *meltype_tsf_capture_resume(void *context, void *composition, uint32_t cookie);
int32_t meltype_tsf_resume_matches(void *snapshot, void *context);
void *meltype_tsf_try_resume(void *snapshot, void *context, void *sink, uint32_t cookie);
/* Adopt the result before styling, so reentrant termination finds its owner.
   This call retains context/composition across host callbacks. */
void meltype_tsf_resume_attribute(void *context, void *composition, uint32_t cookie, uint32_t input_atom);
void meltype_tsf_destroy_resume(void *snapshot);
/* Thread-affine old-composition queue, max eight retained entries. add borrows
   interfaces and schedules a Rust edit session that pins owner through IUnknown.
   end deactivates/drains and requests synchronous cleanup; queued sessions can
   still finish after refusal. reset starts a new activation; destroy is final.
   All queued sessions must execute/release on their creating TSF thread. */
void *meltype_tsf_abandoned_create(void);
void meltype_tsf_abandoned_reset(void *queue);
void meltype_tsf_abandoned_destroy(void *queue);
int32_t meltype_tsf_abandoned_add(void *queue, void *context, void *composition, void *owner, uint32_t client);
int32_t meltype_tsf_abandoned_forget(void *queue, void *composition);
int32_t meltype_tsf_abandoned_end(void *queue, void *owner, uint32_t client);
void meltype_tsf_clear_composition_attribute(void *composition, uint32_t cookie);
/* Read metadata only: secret=1, private/no-learning=2, unreadable=4;
   property sources (Windows=1, app=2)<<8, unreadable step (QI=1,list=2,
   VARIANT=3)<<16. Missing metadata is unspecified; unreadable values are secret.
   Borrowed context/thread-manager pointers, valid cookie, creating TSF thread. */
uint32_t meltype_tsf_input_scope(void *context, uint32_t cookie);
int32_t meltype_tsf_context_disabled(void *context);
/* Read: 0/1, -1 on unavailable value, -2 for unsupported manager.
   Write: 1 on success, 0 on failure. */
int32_t meltype_tsf_read_open(void *thread_manager);
int32_t meltype_tsf_write_open(void *thread_manager, uint32_t client, int32_t open);
/* Thread-affine subscriptions: 0 thread events, 1 foreground keys, 2 mode
   (initially open), 3 top-context edits. Owner is thread manager except kind 3
   (document manager). Sink implements that event interface. Kind 3 requires
   context_out, returning an owned ITfContext on success; otherwise NULL.
   Unsubscribe releases the retained original source, not a fresh lookup. */
void *meltype_tsf_subscribe(uint32_t kind, void *owner, uint32_t client, void *sink, void **context_out);
void meltype_tsf_unsubscribe(void *token);
/* Synchronous edit-event callbacks. lifetime is a borrowed IUnknown retained
   across dispatch. current returns borrowed composition and writes its context.
   query/moved/commit implement owner actions; commit preserves the app caret.
   All callback code/state must remain valid until return; never throw. */
typedef struct MeltypeTsfEditCallbacks {
    void *owner;
    void *lifetime;
    int32_t (*take_own)(void *);
    void *(*current)(void *, void **context);
    void (*query)(void *, uint32_t cookie, void *context, int32_t changed);
    void (*moved)(void *, int32_t reset_english);
    void (*commit)(void *);
} MeltypeTsfEditCallbacks;
int32_t meltype_tsf_edit_notification(void *context, void *record, uint32_t cookie, const MeltypeTsfEditCallbacks *callbacks);
#endif
#ifdef __cplusplus
}
#endif
#endif
