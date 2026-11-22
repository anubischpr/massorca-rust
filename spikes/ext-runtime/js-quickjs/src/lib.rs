use rquickjs::function::Opt;
use rquickjs::{Coerced, Context, Ctx, Function, IntoJs, Runtime, Value};

wit_bindgen::generate!({ path: "../wit", world: "js-engine" });

use massorca::spike::host;

struct Engine;

// Missing or undefined arguments stringify as "undefined", matching the Boa guest.
fn arg_text(arg: Opt<Coerced<String>>) -> String {
    arg.0.map_or_else(|| "undefined".to_owned(), |Coerced(text)| text)
}

fn log(msg: Opt<Coerced<String>>) {
    host::log(&arg_text(msg));
}

fn kv_get<'js>(ctx: Ctx<'js>, key: Opt<Coerced<String>>) -> rquickjs::Result<Value<'js>> {
    match host::kv_get(&arg_text(key)) {
        Some(value) => value.into_js(&ctx),
        None => Ok(Value::new_null(ctx)),
    }
}

fn exception_text(ctx: &Ctx<'_>) -> String {
    match ctx.catch().get::<Coerced<String>>() {
        Ok(Coerced(text)) => text,
        Err(err) => err.to_string(),
    }
}

fn eval_to_string(ctx: &Ctx<'_>, source: String) -> Result<String, String> {
    match ctx.eval::<Coerced<String>, _>(source) {
        Ok(Coerced(text)) => Ok(text),
        Err(rquickjs::Error::Exception) => Err(exception_text(ctx)),
        Err(err) => Err(err.to_string()),
    }
}

impl Guest for Engine {
    fn run(source: String) -> Result<String, String> {
        let runtime = Runtime::new().map_err(|e| e.to_string())?;
        let context = Context::full(&runtime).map_err(|e| e.to_string())?;
        context.with(|ctx| {
            let globals = ctx.globals();
            let log_fn = Function::new(ctx.clone(), log).map_err(|e| e.to_string())?;
            globals.set("log", log_fn).map_err(|e| e.to_string())?;
            let kv_fn = Function::new(ctx.clone(), kv_get).map_err(|e| e.to_string())?;
            globals.set("kvGet", kv_fn).map_err(|e| e.to_string())?;
            eval_to_string(&ctx, source)
        })
    }
}

export!(Engine);
