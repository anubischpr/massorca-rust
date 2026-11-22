// Spike probe: builds a Wasmtime 51 engine with component-model async enabled
// and checks that the WASIp3 module of wasmtime-wasi resolves. Not product code.
use wasmtime::{Config, Engine};
use wasmtime_wasi::p3::bindings as _;

fn main() -> wasmtime::Result<()> {
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.wasm_component_model_async(true);
    let _engine = Engine::new(&config)?;
    println!("engine ok: wasm_component_model=true wasm_component_model_async=true");
    println!("wasmtime_wasi::p3::bindings resolves (p3 feature on)");
    Ok(())
}
