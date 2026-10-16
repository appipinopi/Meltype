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
void meltype_tsf_candidate_hide(void *);
int32_t meltype_tsf_candidate_visible(void *);
void meltype_tsf_candidate_destroy(void *);
#endif
#ifdef __cplusplus
}
#endif
#endif
