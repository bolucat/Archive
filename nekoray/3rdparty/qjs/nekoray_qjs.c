#include "nekoray_qjs.h"
#include "quickjs-libc.h"

#include <string.h>

static JSContext *JS_NewCustomContext(JSRuntime *rt) {
    JSContext *ctx = JS_NewContextRaw(rt);
    if (!ctx)
        return NULL;
    JS_AddIntrinsicBaseObjects(ctx);
    JS_AddIntrinsicDate(ctx);
    JS_AddIntrinsicEval(ctx);
    JS_AddIntrinsicStringNormalize(ctx);
    JS_AddIntrinsicRegExp(ctx);
    JS_AddIntrinsicJSON(ctx);
    JS_AddIntrinsicProxy(ctx);
    JS_AddIntrinsicMapSet(ctx);
    JS_AddIntrinsicTypedArrays(ctx);
    JS_AddIntrinsicPromise(ctx);
    JS_AddIntrinsicBigInt(ctx);
    return ctx;
}

// start of nekoray

void nekoray_qjs_new(nekoray_qjs_new_arg arg) {
    JSRuntime *rt = JS_NewRuntime();
    js_std_set_worker_new_context_func(JS_NewCustomContext);
    js_std_init_handlers(rt);
    JS_SetModuleLoaderFunc(rt, NULL, js_module_loader, NULL);
    JSContext *ctx = JS_NewCustomContext(rt);
    js_std_add_helpers(ctx, 0, NULL);

    if (arg.enable_std) {
        {
            js_init_module_std(ctx, "std");
        }
        {
            js_init_module_os(ctx, "os");
        }
        const char *str =
            "import * as std from 'std';\n"
            "import * as os from 'os';\n"
            "globalThis.std = std;\n"
            "globalThis.os = os;\n";
        JSValue v = JS_Eval(ctx, str, strlen(str), "<std>", JS_EVAL_TYPE_MODULE);
        JS_FreeValue(ctx, v);
    }

    // nekoray func
    JSValue global_obj = JS_GetGlobalObject(ctx);
    JSValue nekoray = JS_NewObject(ctx);
    JS_SetPropertyStr(ctx, nekoray, "log", JS_NewCFunction(ctx, arg.func_log, "log", 1));
    JS_SetPropertyStr(ctx, global_obj, "nekoray", nekoray);
    JS_FreeValue(ctx, global_obj);

    arg.neko_ctx->rt = rt;
    arg.neko_ctx->ctx = ctx;
}

void nekoray_qjs_free(const nekoray_qjs_context *neko_ctx) {
    js_std_free_handlers(neko_ctx->rt);
    JS_FreeContext(neko_ctx->ctx);
    JS_FreeRuntime(neko_ctx->rt);
}

JSValue nekoray_qjs_eval(const nekoray_qjs_context *neko_ctx, const char *input, size_t input_len) {
    JSValue result = JS_Eval(neko_ctx->ctx, input, input_len, "<nekoray_qjs>", 0);
    return result;
}
