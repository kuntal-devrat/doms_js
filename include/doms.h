/*
 * DOMS JavaScript Engine — C Embedding API
 *
 * Copyright (c) 2026 DOMS Contributors
 * Licensed under MIT OR Apache-2.0
 */

#ifndef DOMS_H
#define DOMS_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ABI Version */
#define DOMS_ABI_VERSION 1

/**
 * Return the engine ABI version.
 */
uint32_t doms_version(void);

/* ----------------------------------------------------------------------------
 * Opaque Handles
 * ---------------------------------------------------------------------------- */

typedef struct doms_engine_t   doms_engine_t;   /* Process-global engine  */
typedef struct doms_isolate_t  doms_isolate_t;  /* Independent JS world   */
typedef struct doms_context_t  doms_context_t;  /* Execution context      */
typedef struct doms_value_t    doms_value_t;    /* JS value handle        */
typedef struct doms_script_t   doms_script_t;   /* Compiled script        */
typedef struct doms_module_t   doms_module_t;   /* ES module              */

/* ----------------------------------------------------------------------------
 * Status / Error Codes
 * ---------------------------------------------------------------------------- */

typedef enum {
    DOMS_OK = 0,
    DOMS_ERROR_COMPILE = 1,
    DOMS_ERROR_RUNTIME = 2,
    DOMS_ERROR_OOM = 3,
    DOMS_ERROR_TYPE = 4,
    DOMS_ERROR_INVALID_ARG = 5,
} doms_status_t;

/* ----------------------------------------------------------------------------
 * Engine & Isolate Lifecycle
 * ---------------------------------------------------------------------------- */

/**
 * Create a process-global engine instance.
 */
doms_engine_t*  doms_engine_new(void);

/**
 * Destroy an engine instance and free its resources.
 */
void            doms_engine_destroy(doms_engine_t* engine);

/**
 * Create a new isolate with its own independent heap and runtime.
 */
doms_isolate_t* doms_isolate_new(doms_engine_t* engine);

/**
 * Destroy an isolate and reclaim all its GC heap memory.
 */
void            doms_isolate_destroy(doms_isolate_t* isolate);

/**
 * Create a new execution context within an isolate.
 */
doms_context_t* doms_context_new(doms_isolate_t* isolate);

/**
 * Destroy an execution context.
 */
void            doms_context_destroy(doms_context_t* ctx);

/* ----------------------------------------------------------------------------
 * Script Execution
 * ---------------------------------------------------------------------------- */

/**
 * Parse, compile, and execute a JavaScript source string.
 */
doms_status_t   doms_eval(doms_context_t* ctx,
                          const char* source, size_t len,
                          doms_value_t** result);

/**
 * Pre-compile a JavaScript source string into a bytecode script.
 */
doms_status_t   doms_compile(doms_context_t* ctx,
                             const char* source, size_t len,
                             doms_script_t** script);

/**
 * Execute a pre-compiled script.
 */
doms_status_t   doms_run(doms_context_t* ctx,
                         doms_script_t* script,
                         doms_value_t** result);

/**
 * Destroy a pre-compiled script.
 */
void            doms_script_destroy(doms_script_t* script);

/* ----------------------------------------------------------------------------
 * Value Operations
 * ---------------------------------------------------------------------------- */

doms_status_t   doms_value_to_string(doms_context_t* ctx,
                                     doms_value_t* val,
                                     const char** out, size_t* out_len);

doms_status_t   doms_value_to_number(doms_context_t* ctx,
                                     doms_value_t* val,
                                     double* out);

doms_status_t   doms_value_to_boolean(doms_context_t* ctx,
                                      doms_value_t* val,
                                      int* out);


int             doms_value_is_null(doms_value_t* val);
int             doms_value_is_undefined(doms_value_t* val);
int             doms_value_is_boolean(doms_value_t* val);
int             doms_value_is_number(doms_value_t* val);
int             doms_value_is_string(doms_value_t* val);
int             doms_value_is_object(doms_value_t* val);
int             doms_value_is_function(doms_value_t* val);

/* Value constructors */
doms_value_t*   doms_value_new_undefined(void);
doms_value_t*   doms_value_new_null(void);
doms_value_t*   doms_value_new_boolean(int b);
doms_value_t*   doms_value_new_number(double n);
doms_status_t   doms_value_new_string(doms_context_t* ctx,
                                      const char* str, size_t len,
                                      doms_value_t** result);

/* Value memory management */
void            doms_value_ref(doms_value_t* val);
void            doms_value_unref(doms_value_t* val);
void            doms_free_string(doms_context_t* ctx, const char* str);

/* ----------------------------------------------------------------------------
 * Global Object Operations
 * ---------------------------------------------------------------------------- */

doms_status_t   doms_get_global(doms_context_t* ctx,
                                doms_value_t** result);

doms_status_t   doms_set_global(doms_context_t* ctx,
                                const char* name, size_t name_len,
                                doms_value_t* val);

/* ----------------------------------------------------------------------------
 * Object Operations
 * ---------------------------------------------------------------------------- */

doms_status_t   doms_object_new(doms_context_t* ctx,
                                doms_value_t** result);

doms_status_t   doms_object_get(doms_context_t* ctx,
                                doms_value_t* obj,
                                const char* key, size_t key_len,
                                doms_value_t** result);

doms_status_t   doms_object_set(doms_context_t* ctx,
                                doms_value_t* obj,
                                const char* key, size_t key_len,
                                doms_value_t* value);

/* ----------------------------------------------------------------------------
 * Array Operations
 * ---------------------------------------------------------------------------- */

doms_status_t   doms_array_new(doms_context_t* ctx,
                               doms_value_t** result);

doms_status_t   doms_array_push(doms_context_t* ctx,
                                doms_value_t* arr,
                                doms_value_t* val);

doms_status_t   doms_array_length(doms_context_t* ctx,
                                  doms_value_t* arr,
                                  size_t* out_len);

doms_status_t   doms_array_get_at(doms_context_t* ctx,
                                  doms_value_t* arr,
                                  size_t index,
                                  doms_value_t** result);

/* ----------------------------------------------------------------------------
 * Function Calls and Native Bindings
 * ---------------------------------------------------------------------------- */

doms_status_t   doms_call(doms_context_t* ctx,
                          doms_value_t* func,
                          doms_value_t* this_val,
                          size_t argc, doms_value_t** argv,
                          doms_value_t** result);

typedef doms_status_t (*doms_native_fn)(doms_context_t* ctx,
                                        doms_value_t* this_val,
                                        size_t argc,
                                        doms_value_t** argv,
                                        doms_value_t** result,
                                        void* userdata);

doms_status_t   doms_set_native_function(doms_context_t* ctx,
                                         doms_value_t* obj,
                                         const char* name, size_t name_len,
                                         doms_native_fn fn,
                                         void* userdata);

doms_status_t   doms_set_global_function(doms_context_t* ctx,
                                         const char* name, size_t name_len,
                                         doms_native_fn fn,
                                         void* userdata);

/* ----------------------------------------------------------------------------
 * Error Diagnostics
 * ---------------------------------------------------------------------------- */

const char*     doms_get_error_message(doms_context_t* ctx);

/* ----------------------------------------------------------------------------
 * Parallelism Primitives (Unique to DOMS)
 * ---------------------------------------------------------------------------- */

doms_status_t   doms_isolate_post_message(doms_isolate_t* target,
                                          const uint8_t* data,
                                          size_t len);

typedef void (*doms_message_handler)(doms_isolate_t* isolate,
                                     const uint8_t* data,
                                     size_t len,
                                     void* userdata);

doms_status_t   doms_isolate_set_message_handler(doms_isolate_t* isolate,
                                                 doms_message_handler handler,
                                                 void* userdata);

#ifdef __cplusplus
}
#endif

#endif /* DOMS_H */
