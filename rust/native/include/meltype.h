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
/* Key flags: modifier=1, Japanese toggle=2, Alt-grave toggle=4, composition
   Ctrl shortcut=8. Modifier bits: Shift=1,Ctrl=2,Alt=4,Win=8. Keyboard reads and
   translation use the caller's thread/layout; translation preserves dead keys. */
uint32_t meltype_tsf_key_flags(uint32_t key, uint64_t message, uint32_t modifiers);
uint32_t meltype_tsf_key_modifiers(void);
int32_t meltype_tsf_key_down(int32_t key);
uint32_t meltype_tsf_key_char(uint32_t key, uint64_t message);
typedef struct MeltypeTsfKeySnapshot {
    void *composition, *context;
    int32_t disabled, open, detection, english;
    uint32_t stale_tries, stale_vk;
    uint64_t stale_time;
} MeltypeTsfKeySnapshot;
typedef struct MeltypeTsfKeyCallbacks {
    void *owner, *lifetime;
    void (*read)(void *, MeltypeTsfKeySnapshot *);
    void (*update)(void *, uint32_t operation, uint64_t a, uint64_t b);
    void (*resume)(void *, void *context);
    int32_t (*commit_now)(void *, int32_t keep_caret);
    void (*commit_async)(void *, int32_t keep_caret);
    void (*abandon)(void *);
    int32_t (*ready)(void *);
    int32_t (*secret)(void *, uint32_t key, void *context, int32_t test);
    void (*pass_reason)(void *, uint32_t reason);
} MeltypeTsfKeyCallbacks;
/* Synchronous acceptance. Borrowed context and lifetime IUnknown on the TSF
   thread, writable UTF-16 output, callbacks/state/code valid until return.
   Callbacks never throw. update: 0 reset flags,1 secret while composing,
   2 clear English,3 stale tries/vk packed high/low32 in a and time b,
   4 checked=a. Pass: 0 clear,1 English,2 modifiers,3 disabled,4 continuing
   English,5 core unavailable,6 password. Secret: 0 ordinary,1 secret,-1 unknown. */
int32_t meltype_tsf_would_eat(void *context, uint32_t key, uint64_t message, int32_t test, const MeltypeTsfKeyCallbacks *callbacks, uint16_t *character);
/* Thread-affine key state: owns cached context and replay queue. Reset on a new
   activation; clear cache on focus/deactivation. Destroy on its creating thread. */
void *meltype_tsf_key_state_create(void);
void meltype_tsf_key_state_reset(void *);
void meltype_tsf_key_state_destroy(void *);
void meltype_tsf_key_cache_clear(void *);
/* Reader receives borrowed owner/context and writes private/no-learning. Owner
   lifetime is a borrowed IUnknown retained across the call. Reuse one test-key
   result for the same context/key within 200ms; never throw from callbacks. */
int32_t meltype_tsf_key_secret(void *state, void *context, uint32_t key, int32_t test,
    void *owner, void *lifetime, int32_t (*reader)(void *, void *, int32_t *private_field), int32_t *private_field);
int32_t meltype_tsf_is_reinjected(void *state, uint32_t key);
/* Return 1 sent, 2 loop suppressed, 0 send failed, -1 invalid. Sender borrows
   two Windows INPUTs; NULL uses SendInput, nonnull substitutes transport.
   Tests use substitutes and inject no keys. Optional error receives last-error.
   All pointers/callbacks valid until return on the token's TSF thread. */
struct tagINPUT;
int32_t meltype_tsf_reinject(void *state, uint32_t key, void *lifetime, void *owner,
    uint32_t (*sender)(void *, const struct tagINPUT *inputs, uint32_t count), uint32_t *error);
typedef struct MeltypeTsfKeyStale {uint32_t tries,key;uint64_t time;} MeltypeTsfKeyStale;
typedef struct MeltypeTsfKeyPassed {int32_t key,control;} MeltypeTsfKeyPassed;
void meltype_tsf_key_foreground(void *state, int32_t foreground);
void meltype_tsf_key_stale_read(void *state, MeltypeTsfKeyStale *out);
void meltype_tsf_key_stale_write(void *state, uint32_t tries, uint32_t key, uint64_t time);
void meltype_tsf_key_stale_clear(void *state);
/* mode 0 consumes pending key metadata once; mode 1 emits the recent key or
   selection/text movement (0/-1), with mouse-button precedence on selection. */
int32_t meltype_tsf_key_passed(void *state, int32_t mode, int32_t selection_changed, MeltypeTsfKeyPassed *out);
typedef struct MeltypeTsfKeyEntry {
    void *owner,*lifetime;
    int32_t (*would)(void *, void *context, uint32_t key, uint64_t message, int32_t test, uint16_t *character);
    void (*mark)(void *, uint32_t key, int32_t kind);
    int32_t (*composing)(void *);
    void (*toggle)(void *, void *context);
    void (*handle)(void *, void *context, uint32_t key, uint16_t character);
} MeltypeTsfKeyEntry;
/* kind: 0 test down,1 down,2 test up,3 up. Returns consumed. mark kind 1 is
   replay passthrough (clear English/mark moved), 2 ordinary passthrough (moved).
   Native state owns consumed keys, replay test/down pairing and passed metadata.
   Down callbacks/lifetime/context are borrowed on their TSF thread and must
   survive until return; never throw. Up events need no callbacks/context. */
int32_t meltype_tsf_key_event(void *state, uint32_t kind, void *context, uint32_t key, uint64_t message, const MeltypeTsfKeyEntry *entry);
typedef struct MeltypeTsfModeCallbacks {
    void *owner,*lifetime;
    uint32_t (*read)(void *); /* open=1, code application=2, code English=4 */
    int32_t (*ready)(void *);
    int32_t (*secret)(void *, uint32_t cookie, void *context);
    void (*set_open)(void *, int32_t open);
    int32_t (*request)(void *, const char *json); /* apply reply, return active && consumed */
    void (*notice)(void *, int32_t japanese);
} MeltypeTsfModeCallbacks;
/* Borrowed TSF interfaces, valid UTF-8 metadata {sid:string,process:string} and
   non-null callback functions on their TSF thread. All callbacks must not throw.
   Pins owner/context across focus, read-session and core callbacks. Toggle
   resolves focus only for a null context. Code-line null context always fails.
   A refused synchronous read uses the core's tracked line; password fields
   never read surrounding text or send toggle requests. */
int32_t meltype_tsf_toggle_mode(void *context, void *thread_manager, void *key_state, uint32_t client, const char *metadata, const MeltypeTsfModeCallbacks *callbacks);
int32_t meltype_tsf_code_line(void *context, void *key_state, uint32_t client, const char *metadata, const MeltypeTsfModeCallbacks *callbacks, int32_t japanese);
#endif
#ifdef __cplusplus
}
#endif
#endif
