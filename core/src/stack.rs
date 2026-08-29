use std::cell::Cell;

pub const DEEP_STACK_BYTES: usize = 256 * 1024 * 1024;

thread_local! {
    static ALREADY_DEEP: Cell<bool> = const { Cell::new(false) };
}

pub fn is_deep() -> bool {
    ALREADY_DEEP.with(Cell::get)
}

pub fn on_deep_stack<T, F>(work: F) -> T
where
    F: FnOnce() -> T + Send,
    T: Send,
{
    if is_deep() {
        return work();
    }
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("nothing-deep-stack".to_string())
            .stack_size(DEEP_STACK_BYTES)
            .spawn_scoped(scope, || {
                ALREADY_DEEP.with(|flag| flag.set(true));
                work()
            })
            .expect("failed to spawn the deep-stack worker");
        match worker.join() {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_runs_once_and_hands_its_answer_back() {
        assert_eq!(on_deep_stack(|| 6 * 7), 42);
    }

    #[test]
    fn a_nested_call_stays_on_the_stack_it_is_already_on() {
        let (outer, inner) = on_deep_stack(|| {
            let outer = is_deep();
            let inner = on_deep_stack(is_deep);
            (outer, inner)
        });
        assert!(outer, "the worker knows it is the deep one");
        assert!(inner, "and a nested call does not spawn a second worker");
        assert!(!is_deep(), "the caller's own thread is left as it was");
    }

    #[test]
    #[should_panic(expected = "from the deep worker")]
    fn a_panic_inside_the_worker_reaches_the_caller() {
        on_deep_stack(|| panic!("from the deep worker"));
    }

    #[test]
    fn a_recursion_far_past_a_two_megabyte_stack_completes() {
        fn descend(n: usize) -> usize {
            let mut padding = [0u8; 256];
            padding[0] = (n % 251) as u8;
            if n == 0 {
                padding[0] as usize
            } else {
                descend(n - 1) + padding[0] as usize
            }
        }
        let answer = std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(|| on_deep_stack(|| descend(100_000)))
            .expect("spawn the small-stack caller")
            .join()
            .expect("the small-stack caller finished");
        assert!(answer > 0);
    }
}
