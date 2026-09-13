//! Deterministic, cooperative Luau task scheduling.
//!
//! Tasks are Lua coroutines rather than Rust futures. This keeps the contract
//! identical for the native `mlua` host and the browser `luaur-rt` host, and
//! means waiting advances with simulation time instead of wall-clock time.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use super::lua;
#[cfg(target_arch = "wasm32")]
use luaur_rt::ThreadStatus;
#[cfg(not(target_arch = "wasm32"))]
use mlua::thread::ThreadStatus;

pub(super) const MAX_TASKS: usize = 1_024;
pub(super) const MAX_TASKS_PER_TICK: u32 = 128;
const MAX_TASK_DELAY: f32 = 24.0 * 60.0 * 60.0;
const WAIT_TAG: &str = "__cubacadabra_task_wait";

type TaskId = u64;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Queue {
    Immediate,
    Deferred,
    Sleeping { wake_at: f32 },
    Running,
}

struct Task {
    thread: lua::Thread,
    queue: Queue,
    resume_value: Option<f32>,
}

pub(super) struct TaskRun {
    pub(super) id: TaskId,
    pub(super) thread: lua::Thread,
    pub(super) resume_value: Option<f32>,
}

pub(super) struct TaskScheduler {
    next_id: TaskId,
    now: f32,
    tasks: BTreeMap<TaskId, Task>,
    immediate: VecDeque<TaskId>,
    deferred: VecDeque<TaskId>,
    executions_this_tick: u32,
}

pub(super) fn install(
    lua: &lua::Lua,
    api: &lua::Table,
    scheduler: Rc<RefCell<TaskScheduler>>,
) -> lua::Result<()> {
    let task = super::create_table(lua)?;
    lua.globals().set("task", task.clone())?;

    let spawn_scheduler = Rc::clone(&scheduler);
    task.set(
        "spawn",
        lua.create_function(move |lua_state, args: lua::Variadic<lua::Value>| {
            let callback = function_arg(&args)
                .ok_or_else(|| lua::Error::runtime("task.spawn expects a callback"))?;
            spawn_scheduler
                .borrow_mut()
                .spawn(lua_state, callback, Queue::Immediate)
                .map_err(lua::Error::runtime)
        })?,
    )?;

    let defer_scheduler = Rc::clone(&scheduler);
    task.set(
        "defer",
        lua.create_function(move |lua_state, args: lua::Variadic<lua::Value>| {
            let callback = function_arg(&args)
                .ok_or_else(|| lua::Error::runtime("task.defer expects a callback"))?;
            defer_scheduler
                .borrow_mut()
                .spawn(lua_state, callback, Queue::Deferred)
                .map_err(lua::Error::runtime)
        })?,
    )?;

    let delay_scheduler = Rc::clone(&scheduler);
    task.set(
        "delay",
        lua.create_function(move |lua_state, args: lua::Variadic<lua::Value>| {
            let seconds = args
                .iter()
                .find_map(lua_number)
                .ok_or_else(|| lua::Error::runtime("task.delay expects seconds"))?;
            let callback = function_arg(&args)
                .ok_or_else(|| lua::Error::runtime("task.delay expects a callback"))?;
            delay_scheduler
                .borrow_mut()
                .delay(lua_state, seconds, callback)
                .map_err(lua::Error::runtime)
        })?,
    )?;

    let cancel_scheduler = scheduler;
    task.set(
        "cancel",
        lua.create_function(move |_, args: lua::Variadic<lua::Value>| {
            let id = args
                .iter()
                .find_map(task_id)
                .ok_or_else(|| lua::Error::runtime("task.cancel expects a task handle"))?;
            Ok(cancel_scheduler.borrow_mut().cancel(id))
        })?,
    )?;

    // Keep wait in Luau rather than a Rust callback: coroutine.yield must not
    // cross a synchronous C callback boundary, and this exact source works in
    // both mlua/Luau and luaur-rt.
    let wait: lua::Function = lua
        .load(
            r#"
                return function(first, second)
                    local seconds = second
                    if seconds == nil then
                        seconds = type(first) == "table" and 0 or first
                    end
                    return coroutine.yield("__cubacadabra_task_wait", seconds or 0)
                end
            "#,
        )
        .eval()?;
    task.set("wait", wait)?;
    api.set("task", task)
}

fn function_arg(args: &[lua::Value]) -> Option<lua::Function> {
    args.iter().find_map(|value| match value {
        lua::Value::Function(function) => Some(function.clone()),
        _ => None,
    })
}

fn task_id(value: &lua::Value) -> Option<TaskId> {
    match value {
        lua::Value::Integer(value) => (*value).try_into().ok(),
        lua::Value::Number(value) if value.is_finite() && *value >= 0.0 && value.fract() == 0.0 => {
            Some(*value as u64)
        }
        _ => None,
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self {
            next_id: 1,
            now: 0.0,
            tasks: BTreeMap::new(),
            immediate: VecDeque::new(),
            deferred: VecDeque::new(),
            executions_this_tick: 0,
        }
    }
}

impl TaskScheduler {
    fn spawn(
        &mut self,
        lua: &lua::Lua,
        callback: lua::Function,
        queue: Queue,
    ) -> Result<TaskId, String> {
        if self.tasks.len() >= MAX_TASKS {
            return Err("task scheduler is full".to_owned());
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        let thread = lua
            .create_thread(callback)
            .map_err(|error| error.to_string())?;
        self.tasks.insert(
            id,
            Task {
                thread,
                queue,
                resume_value: None,
            },
        );
        self.enqueue(id, queue);
        Ok(id)
    }

    fn delay(
        &mut self,
        lua: &lua::Lua,
        seconds: f32,
        callback: lua::Function,
    ) -> Result<TaskId, String> {
        if !seconds.is_finite() || !(0.0..=MAX_TASK_DELAY).contains(&seconds) {
            return Err(format!(
                "task delay must be finite and between 0 and {MAX_TASK_DELAY} seconds"
            ));
        }
        let queue = if seconds == 0.0 {
            Queue::Immediate
        } else {
            Queue::Sleeping {
                wake_at: self.now + seconds,
            }
        };
        self.spawn(lua, callback, queue)
    }

    fn cancel(&mut self, id: TaskId) -> bool {
        self.tasks.remove(&id).is_some()
    }

    pub(super) fn begin_tick(&mut self, delta: f32) {
        self.now += delta.max(0.0);
        self.executions_this_tick = 0;

        // Deferred work is promoted before newly due delayed work. Existing
        // immediate work (including work left by the previous budget) remains
        // first, preserving FIFO ordering across a budget boundary.
        while let Some(id) = self.deferred.pop_front() {
            if self.mark_queued(id, Queue::Immediate) {
                self.immediate.push_back(id);
            }
        }

        let due = self
            .tasks
            .iter()
            .filter_map(|(&id, task)| match task.queue {
                Queue::Sleeping { wake_at } if wake_at <= self.now => Some(id),
                _ => None,
            })
            .collect::<Vec<_>>();
        for id in due {
            if let Some(task) = self.tasks.get_mut(&id) {
                if let Queue::Sleeping { wake_at } = task.queue
                    && let Some(requested) = task.resume_value
                {
                    // For task.wait, resume with the actual elapsed time.
                    // Delayed callbacks have no resume value and resume with
                    // no arguments.
                    task.resume_value = Some((self.now - (wake_at - requested)).max(0.0));
                }
                task.queue = Queue::Immediate;
            }
            self.immediate.push_back(id);
        }
    }

    pub(super) fn next(&mut self) -> Option<TaskRun> {
        if self.executions_this_tick >= MAX_TASKS_PER_TICK {
            return None;
        }
        while let Some(id) = self.immediate.pop_front() {
            let Some(task) = self.tasks.get_mut(&id) else {
                continue;
            };
            if task.queue != Queue::Immediate {
                continue;
            }
            task.queue = Queue::Running;
            self.executions_this_tick += 1;
            return Some(TaskRun {
                id,
                thread: task.thread.clone(),
                resume_value: task.resume_value.take(),
            });
        }
        None
    }

    pub(super) fn finish(
        &mut self,
        run: TaskRun,
        result: Result<lua::MultiValue, lua::Error>,
        budget_interrupted: bool,
    ) -> Option<String> {
        let Some(task) = self.tasks.get_mut(&run.id) else {
            // The task canceled itself while it was running.
            return None;
        };

        let values = match result {
            Ok(values) => values,
            Err(error) => {
                self.tasks.remove(&run.id);
                return Some(format!("task {} failed: {}", run.id, error));
            }
        };

        match run.thread.status() {
            ThreadStatus::Finished => {
                self.tasks.remove(&run.id);
                None
            }
            ThreadStatus::Resumable => {
                let yielded = values.into_iter().collect::<Vec<_>>();
                if budget_interrupted && yielded.is_empty() {
                    // An instruction budget yield is resumed on the next
                    // simulation tick, even if the task did not call wait.
                    task.queue = Queue::Immediate;
                    self.immediate.push_back(run.id);
                    return None;
                }
                if yielded.len() == 2
                    && matches!(&yielded[0], lua::Value::String(tag) if tag.to_string_lossy() == WAIT_TAG)
                {
                    let Some(seconds) = lua_number(&yielded[1]) else {
                        self.tasks.remove(&run.id);
                        return Some(format!("task {} yielded an invalid wait duration", run.id));
                    };
                    if !seconds.is_finite() || !(0.0..=MAX_TASK_DELAY).contains(&seconds) {
                        self.tasks.remove(&run.id);
                        return Some(format!("task {} yielded an invalid wait duration", run.id));
                    }
                    task.queue = Queue::Sleeping {
                        wake_at: self.now + seconds,
                    };
                    task.resume_value = Some(seconds);
                    return None;
                }

                self.tasks.remove(&run.id);
                Some(format!(
                    "task {} yielded without task.wait; use task.wait(seconds)",
                    run.id
                ))
            }
            status => {
                self.tasks.remove(&run.id);
                Some(format!("task {} became unresumable ({status:?})", run.id))
            }
        }
    }

    fn enqueue(&mut self, id: TaskId, queue: Queue) {
        match queue {
            Queue::Immediate => self.immediate.push_back(id),
            Queue::Deferred => self.deferred.push_back(id),
            Queue::Sleeping { .. } | Queue::Running => {}
        }
    }

    fn mark_queued(&mut self, id: TaskId, queue: Queue) -> bool {
        let Some(task) = self.tasks.get_mut(&id) else {
            return false;
        };
        task.queue = queue;
        true
    }
}

fn lua_number(value: &lua::Value) -> Option<f32> {
    match value {
        lua::Value::Integer(value) => Some(*value as f32),
        lua::Value::Number(value) => Some(*value as f32),
        _ => None,
    }
}
