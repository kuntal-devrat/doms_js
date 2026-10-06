# Chapter 9: Embedding & C ABI (`libdoms.h`)

One of DOMS's core design mandates is providing a rock-solid, production-grade **C ABI** (`doms_ffi`).

---

## 1. Design Principles

- **Opaque Pointers:** All engine structures are exposed as opaque pointer types (`doms_engine_t*`, `doms_isolate_t*`, `doms_context_t*`, `doms_value_t*`). Internal changes never alter struct layout for consumers.
- **Explicit Ownership:** Handles are managed through explicit reference counting (`doms_value_ref` / `doms_value_unref`).
- **Error Codes:** Functions return explicit `doms_status_t` enums with inspectable error strings.
- **ABI Stability Guarantee:** The `DOMS_ABI_VERSION` constant ensures compile-time and runtime compatibility.

---

## 2. Complete C Embedding Example

Below is a complete, working example embedding DOMS into a C program:

```c
#include "doms.h"
#include <stdio.h>

int main(void) {
    // 1. Initialize Engine and Isolate
    doms_engine_t* engine = doms_engine_new();
    doms_isolate_t* isolate = doms_isolate_new(engine);
    doms_context_t* ctx = doms_context_new(isolate);

    // 2. Evaluate JavaScript code
    const char* code = "let x = 10; let y = 32; x + y;";
    doms_value_t* result = NULL;
    doms_status_t status = doms_eval(ctx, code, strlen(code), &result);

    if (status == DOMS_OK) {
        double num = 0.0;
        doms_value_to_number(ctx, result, &num);
        printf("Result from JS: %.0f\n", num); // Prints 42
        doms_value_unref(result);
    } else {
        printf("Evaluation failed: %s\n", doms_get_error_message(ctx));
    }

    // 3. Clean up
    doms_context_destroy(ctx);
    doms_isolate_destroy(isolate);
    doms_engine_destroy(engine);

    return 0;
}
```

---

## 3. Native Function Binding

Host applications can register native C callbacks into JavaScript:

```c
doms_status_t my_native_print(doms_context_t* ctx,
                              doms_value_t* this_val,
                              size_t argc,
                              doms_value_t** argv,
                              doms_value_t** result,
                              void* userdata) {
    printf("Invoked from JavaScript!\n");
    *result = doms_value_new_undefined();
    return DOMS_OK;
}
```
