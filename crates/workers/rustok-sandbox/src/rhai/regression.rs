//! Regression coverage for rhai 1.27 behavior that the RusToK sandbox relies
//! on. Each test names the upstream release change it pins; remove or adjust
//! the gates together with the workspace `rhai` requirement in `Cargo.toml`.

use rhai::{Dynamic, Engine, EvalAltResult, FnPtr, NativeCallContext, Position};

/// Under the `sync` feature, a method call on a shared variable whose closure
/// argument captures the same variable must fail with `ErrorDataRace` instead
/// of panicking while the native function is resolved.
///
/// Upstream: rhai 1.27, PR #1190. Before the fix the `sync` path treated every
/// shared value as unlocked, read the locked receiver's type during function
/// resolution and aborted the worker process.
#[test]
fn shared_receiver_captured_by_closure_reports_data_race() {
    let mut engine = Engine::new();
    engine.register_fn(
        "with_arg",
        |ctx: NativeCallContext, t: i64, f: FnPtr| -> Result<Dynamic, Box<EvalAltResult>> {
            f.call_within_context(&ctx, (t,))
        },
    );

    // The closure takes no argument while `with_arg` passes one, so resolution
    // continues into another function call that reads every argument's type,
    // including the locked shared receiver.
    let err = engine
        .eval::<Dynamic>("let t = 7; t.with_arg(|| t + 1)")
        .expect_err("capturing a locked shared receiver must be a data race");

    assert!(matches!(*err, EvalAltResult::ErrorDataRace(..)), "{err}");
}

/// Errors raised by built-in binary operations must retain their real source
/// position instead of `Position::NONE`.
///
/// Upstream: rhai 1.27 ("errors raised during built-in binary operations such
/// as `100 / 0` now contain proper `Position` information"). The sandbox
/// converts rhai errors into structured `RhaiError` values for execution logs,
/// so positions have to survive the runtime path.
#[test]
fn division_by_zero_error_carries_position() {
    let engine = Engine::new();

    let err = engine
        .eval::<i64>("1 / 0")
        .expect_err("integer division by zero must fail");

    assert!(matches!(*err, EvalAltResult::ErrorArithmetic(..)), "{err}");
    assert_ne!(
        err.position(),
        Position::NONE,
        "binary-operation errors must retain their source position"
    );
}

/// Operation counts produced by scripted callbacks invoked from native Rust
/// functions must reach `Engine::on_progress` instead of being discarded.
///
/// Upstream: rhai 1.27. The sandbox drives wall-clock deadlines and host
/// cancellation through `on_progress`; a callback whose operations were
/// invisible to the counter could run past every configured limit.
#[test]
fn progress_count_advances_inside_native_callback() {
    use std::sync::{Arc, Mutex};

    let counts = Arc::new(Mutex::new(Vec::<u64>::new()));
    let seen = counts.clone();

    let mut engine = Engine::new();
    engine.on_progress(move |count| {
        let mut counts = seen.lock().unwrap();
        counts.push(count);
        // Backstop so a regression cannot turn this test into a hang.
        (counts.len() >= 1_000).then(|| Dynamic::from("stopped"))
    });
    engine.register_fn(
        "run_callback",
        |ctx: NativeCallContext, f: FnPtr| -> Result<Dynamic, Box<EvalAltResult>> {
            f.call_within_context(&ctx, ())
        },
    );

    let err = engine
        .eval::<Dynamic>(
            r#"
                let spin = || {
                    let x = 0;
                    loop {
                        x += 1;
                    }
                };
                run_callback(spin)
            "#,
        )
        .expect_err("the progress interrupt must stop the unbounded callback");

    assert!(
        matches!(*err, EvalAltResult::ErrorTerminated(..)),
        "{err}"
    );

    let counts = counts.lock().unwrap();
    assert!(
        counts.len() >= 1_000,
        "on_progress must keep firing inside the native callback"
    );
    let tail = &counts[counts.len() - 100..];
    assert!(
        tail.windows(2).all(|window| window[1] > window[0]),
        "operation counts observed inside the native callback must advance: {tail:?}"
    );
}
