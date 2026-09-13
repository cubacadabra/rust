return {
  on_interaction = function(api, event)
    -- Persist the callback result into the engine-owned script state so the
    -- run exercises both loading and dispatching the game-script boundary.
    api.lobby:set_status(event.id .. ":" .. event.phase)
  end,

  on_tick = function(api, delta)
  end,
}
