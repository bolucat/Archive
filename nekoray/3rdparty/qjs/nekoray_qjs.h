#include "quickjs.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct nekoray_qjs_context {
    JSRuntime *rt;
    JSContext *ctx;
} nekoray_qjs_context;

typedef struct nekoray_qjs_new_arg {
    nekoray_qjs_context *neko_ctx;
    char enable_std;
    //
    JSValue (*func_log)(JSContext *ctx, JSValue this_val, int argc, JSValue *argv);
} nekoray_qjs_new_arg;

void nekoray_qjs_new(nekoray_qjs_new_arg arg);

void nekoray_qjs_free(const nekoray_qjs_context *neko_ctx);

JSValue nekoray_qjs_eval(const nekoray_qjs_context *neko_ctx, const char *input, size_t input_len);

#ifdef __cplusplus
}
#endif
