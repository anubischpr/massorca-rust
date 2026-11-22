use boa_engine::{Context, JsResult, JsValue, NativeFunction, Source, js_string};

wit_bindgen::generate!({ path: "../wit", world: "js-engine" });

use massorca::spike::host;

struct Engine;

fn log(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let msg = args.first().cloned().unwrap_or_default().to_string(ctx)?;
    host::log(&msg.to_std_string_escaped());
    Ok(JsValue::undefined())
}

fn kv_get(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let key = args.first().cloned().unwrap_or_default().to_string(ctx)?;
    Ok(match host::kv_get(&key.to_std_string_escaped()) {
        Some(v) => JsValue::from(js_string!(v.as_str())),
        None => JsValue::null(),
    })
}

impl Guest for Engine {
    fn run(source: String) -> Result<String, String> {
        let mut ctx = Context::default();
        ctx.register_global_builtin_callable(js_string!("log"), 1, NativeFunction::from_fn_ptr(log))
            .map_err(|e| e.to_string())?;
        ctx.register_global_builtin_callable(js_string!("kvGet"), 1, NativeFunction::from_fn_ptr(kv_get))
            .map_err(|e| e.to_string())?;
        let value = ctx
            .eval(Source::from_bytes(source.as_bytes()))
            .map_err(|e| e.to_string())?;
        value
            .to_string(&mut ctx)
            .map(|s| s.to_std_string_escaped())
            .map_err(|e| e.to_string())
    }
}

export!(Engine);
