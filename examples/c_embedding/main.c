/**
 * DOMS JavaScript Engine — C Embedding Showcase
 *
 * Demonstrates:
 * 1. Initializing the engine, isolate, and execution context.
 * 2. Registering native C host functions into the JavaScript environment.
 * 3. Evaluating JavaScript code from C.
 * 4. Constructing and manipulating JavaScript objects and arrays from C.
 * 5. Calling JavaScript functions directly from C.
 * 6. Clean memory management and error diagnostics.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include "../../crates/doms_ffi/include/doms.h"

/* Native C callback: multiply(a, b) -> number */
static doms_status_t native_multiply(doms_context_t* ctx,
                                     doms_value_t* this_val,
                                     size_t argc,
                                     doms_value_t** argv,
                                     doms_value_t** result,
                                     void* userdata)
{
    (void)this_val;
    (void)userdata;

    if (argc < 2) {
        *result = doms_value_new_undefined();
        return DOMS_OK;
    }

    double a = 0.0, b = 0.0;
    doms_value_to_number(ctx, argv[0], &a);
    doms_value_to_number(ctx, argv[1], &b);

    *result = doms_value_new_number(a * b);
    return DOMS_OK;
}

int main(void)
{
    printf("=== DOMS JavaScript Engine — C Embedding Showcase ===\n");
    printf("DOMS ABI Version: %u\n\n", doms_version());

    /* 1. Lifecycle initialization */
    doms_engine_t* engine = doms_engine_new();
    assert(engine != NULL);

    doms_isolate_t* isolate = doms_isolate_new(engine);
    assert(isolate != NULL);

    doms_context_t* ctx = doms_context_new(isolate);
    assert(ctx != NULL);

    /* 2. Bind native C function: globalThis.multiply(a, b) */
    const char* fn_name = "multiply";
    doms_status_t status = doms_set_global_function(ctx, fn_name, strlen(fn_name), native_multiply, NULL);
    assert(status == DOMS_OK);
    printf("[1] Registered native C function: globalThis.multiply(a, b)\n");

    /* 3. Evaluate script calling native function */
    const char* script = "const res = multiply(7, 6); `Answer is ${res}`";
    doms_value_t* eval_res = NULL;
    status = doms_eval(ctx, script, strlen(script), &eval_res);
    assert(status == DOMS_OK);

    const char* str_out = NULL;
    size_t str_len = 0;
    doms_value_to_string(ctx, eval_res, &str_out, &str_len);
    printf("[2] Evaluated JS: \"%s\"\n    -> Result: \"%s\"\n", script, str_out);
    doms_free_string(ctx, str_out);
    doms_value_unref(eval_res);

    /* 4. Construct JS Object in C */
    doms_value_t* obj = NULL;
    status = doms_object_new(ctx, &obj);
    assert(status == DOMS_OK);

    doms_value_t* host_val = NULL;
    doms_value_new_string(ctx, "DOMS C Host", strlen("DOMS C Host"), &host_val);
    doms_object_set(ctx, obj, "name", 4, host_val);
    doms_value_unref(host_val);

    doms_value_t* speed_val = doms_value_new_number(100.0);
    doms_object_set(ctx, obj, "performance", 11, speed_val);
    doms_value_unref(speed_val);

    doms_set_global(ctx, "hostInfo", 8, obj);
    doms_value_unref(obj);
    printf("[3] Created JS Object from C: hostInfo = { name: 'DOMS C Host', performance: 100 }\n");

    /* 5. Construct JS Array in C */
    doms_value_t* arr = NULL;
    status = doms_array_new(ctx, &arr);
    assert(status == DOMS_OK);

    for (int i = 1; i <= 5; ++i) {
        doms_value_t* item = doms_value_new_number((double)(i * 10));
        doms_array_push(ctx, arr, item);
        doms_value_unref(item);
    }

    size_t arr_len = 0;
    doms_array_length(ctx, arr, &arr_len);
    assert(arr_len == 5);

    doms_set_global(ctx, "numbers", 7, arr);
    doms_value_unref(arr);
    printf("[4] Created JS Array from C: numbers with length %zu\n", arr_len);

    /* 6. Process data in JS and return */
    const char* sum_script = "numbers.reduce((acc, n) => acc + n, 0)";
    doms_value_t* sum_res = NULL;
    status = doms_eval(ctx, sum_script, strlen(sum_script), &sum_res);
    assert(status == DOMS_OK);

    double sum = 0.0;
    doms_value_to_number(ctx, sum_res, &sum);
    printf("[5] JS computed sum of array: %.0f\n", sum);
    doms_value_unref(sum_res);

    /* 7. Call JS function directly from C */
    const char* fn_def = "function greet(target) { return `Hello, ${target.name}!`; }";
    doms_value_t* ignored = NULL;
    doms_eval(ctx, fn_def, strlen(fn_def), &ignored);
    doms_value_unref(ignored);

    doms_value_t* global_obj = NULL;
    doms_get_global(ctx, &global_obj);
    doms_value_t* greet_fn = NULL;
    doms_object_get(ctx, global_obj, "greet", 5, &greet_fn);

    doms_value_t* target_arg = NULL;
    doms_object_get(ctx, global_obj, "hostInfo", 8, &target_arg);

    doms_value_t* call_res = NULL;
    doms_value_t* args[1] = { target_arg };
    status = doms_call(ctx, greet_fn, global_obj, 1, args, &call_res);
    assert(status == DOMS_OK);

    doms_value_to_string(ctx, call_res, &str_out, &str_len);
    printf("[6] Direct C invocation of greet(hostInfo): \"%s\"\n", str_out);
    doms_free_string(ctx, str_out);

    doms_value_unref(call_res);
    doms_value_unref(target_arg);
    doms_value_unref(greet_fn);
    doms_value_unref(global_obj);

    /* 8. Teardown */
    doms_context_destroy(ctx);
    doms_isolate_destroy(isolate);
    doms_engine_destroy(engine);

    printf("\n=== C Embedding Showcase completed successfully! ===\n");
    return 0;
}
