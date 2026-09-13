# Luau task scheduler

Game scripts can schedule cooperative work through `api.task`. The scheduler
uses the engine's fixed simulation ticks; it never sleeps on wall-clock time.

```luau
api.task:spawn(function()
    api.effects:set_state("gate", "open")
    api.task:wait(2)
    api.effects:set_state("gate", "closed")
end)

local animation = api.task:delay(0.25, function()
    -- delayed work
end)
api.task:cancel(animation)

api.task:defer(function()
    -- runs on the next simulation tick
end)
```

`spawn` appends work to the current tick's immediate FIFO queue. `defer`
appends it to the next tick. `delay(seconds, callback)` wakes at the requested
simulation time; a zero delay is immediate. `wait(seconds)` may be used only
inside a scheduled callback and returns the elapsed simulation time when it is
resumed. `cancel` accepts the integer handle returned by `spawn`, `defer`, or
`delay` and returns whether a live task was removed.

Tasks are isolated: an error removes only that task and is reported through the
engine's normal script diagnostic (`last_script_error`). A task that yields
through raw `coroutine.yield` is also removed; use `api.task:wait` so the host
can apply simulation-time ordering.

The scheduler runs at most 128 task resumes per simulation tick. Luau VM
interrupts additionally cap each tick at 100,000 VM safepoints. If the latter
limit is reached, the current task is resumed on the next simulation tick.
These are cooperative limits: code running inside a non-yieldable native call
cannot be interrupted until Luau reaches a safe point.

The native `mlua` and browser `luaur-rt` hosts share this contract and the same
queue ordering: existing immediate work, deferred work promoted at tick start,
then delayed work that became due, with FIFO order within each queue.
