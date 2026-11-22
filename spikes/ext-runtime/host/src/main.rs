//! Loads a JS-engine component and measures size, compile, start, run and memory.
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::{Config, Engine, ResourceLimiter, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

wasmtime::component::bindgen!({ path: "../wit", world: "js-engine" });

const MEMORY_CAP: usize = 128 << 20;

struct State {
    wasi: WasiCtx,
    table: ResourceTable,
    peak: usize,
    host_calls: u64,
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView { ctx: &mut self.wasi, table: &mut self.table }
    }
}

impl massorca::spike::host::Host for State {
    fn log(&mut self, _message: String) {
        self.host_calls += 1;
    }
    fn kv_get(&mut self, key: String) -> Option<String> {
        self.host_calls += 1;
        Some(format!("value-of-{key}"))
    }
}

impl ResourceLimiter for State {
    fn memory_growing(&mut self, _current: usize, desired: usize, _max: Option<usize>) -> wasmtime::Result<bool> {
        if desired > MEMORY_CAP {
            return Ok(false);
        }
        self.peak = self.peak.max(desired);
        Ok(true)
    }
    fn table_growing(&mut self, _current: usize, desired: usize, _max: Option<usize>) -> wasmtime::Result<bool> {
        Ok(desired <= 1 << 20)
    }
}

fn ms(d: Duration) -> String {
    format!("{:.1} ms", d.as_secs_f64() * 1000.0)
}

struct Runner {
    engine: Engine,
    linker: Linker<State>,
    pre: JsEngineIndices,
    component: Component,
}

struct Outcome {
    result: Result<String, String>,
    instantiate: Duration,
    run: Duration,
    peak: usize,
    host_calls: u64,
}

impl Runner {
    fn run(&self, source: &str, deadline_ticks: u64) -> Result<Outcome> {
        let mut store = Store::new(
            &self.engine,
            State { wasi: WasiCtx::builder().build(), table: ResourceTable::new(), peak: 0, host_calls: 0 },
        );
        store.limiter(|s| s);
        store.set_epoch_deadline(deadline_ticks);
        let t = Instant::now();
        let instance = self.linker.instantiate(&mut store, &self.component)?;
        let bindings = self.pre.load(&mut store, &instance)?;
        let instantiate = t.elapsed();
        let t = Instant::now();
        let result = match bindings.call_run(&mut store, source) {
            Ok(r) => r,
            Err(trap) => Err(format!("trap: {}", trap.root_cause())),
        };
        let run = t.elapsed();
        let s = store.data();
        Ok(Outcome { result, instantiate, run, peak: s.peak, host_calls: s.host_calls })
    }
}

fn main() -> Result<()> {
    let path = std::env::args().nth(1).context("usage: spike-host <component.wasm>")?;
    let bytes = std::fs::read(&path)?;
    println!("component size: {:.2} MiB", bytes.len() as f64 / (1 << 20) as f64);

    let mut config = Config::new();
    config.epoch_interruption(true);
    let engine = Engine::new(&config)?;

    let t = Instant::now();
    let compiled = Component::new(&engine, &bytes)?;
    println!("compile (cranelift, cold): {}", ms(t.elapsed()));
    let cwasm = compiled.serialize()?;
    println!("precompiled size: {:.2} MiB", cwasm.len() as f64 / (1 << 20) as f64);
    let t = Instant::now();
    // SAFETY: the bytes were produced by `serialize` on this engine just above.
    #[allow(unsafe_code)]
    let component = unsafe { Component::deserialize(&engine, &cwasm)? };
    println!("load precompiled: {}", ms(t.elapsed()));

    let mut linker = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
    JsEngine::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)?;
    let pre = JsEngineIndices::new(&linker.instantiate_pre(&component)?)?;
    let runner = Runner { engine: engine.clone(), linker, pre, component };

    // Epoch ticker: one tick every 10 ms.
    let ticker = engine.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(10));
        ticker.increment_epoch();
    });

    let cases: [(&str, &str, u64); 6] = [
        ("trivial", "1 + 1", 500),
        ("json 2k items", "let a=[];for(let i=0;i<2000;i++)a.push({id:i,name:'n'+i,tags:['x','y']});JSON.stringify(JSON.parse(JSON.stringify(a))).length", 500),
        ("10k host calls", "let n=0;for(let i=0;i<5000;i++){log('m'+i);if(kvGet('k'+i))n++};n", 500),
        ("closures/classes", "class P{constructor(x){this.x=x}get d(){return this.x*2}};[...Array(20000).keys()].map(i=>new P(i)).filter(p=>p.d%3==0).reduce((s,p)=>s+p.x,0)", 500),
        ("infinite loop, 1 s deadline", "while(true){}", 100),
        ("allocate past 128 MiB", "let a=[];while(true)a.push('x'.repeat(1<<20)+a.length);", 3000),
    ];
    for (name, src, ticks) in cases {
        let o = runner.run(src, ticks)?;
        let shown = match &o.result {
            Ok(v) => format!("ok {}", v.chars().take(40).collect::<String>()),
            Err(e) => format!("err {}", e.chars().take(80).collect::<String>()),
        };
        println!(
            "{name}: instantiate {}, run {}, peak memory {:.1} MiB, host calls {} -> {shown}",
            ms(o.instantiate), ms(o.run), o.peak as f64 / (1 << 20) as f64, o.host_calls
        );
    }
    Ok(())
}
