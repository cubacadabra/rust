#ifndef CUBACADABRA_APP_H
#define CUBACADABRA_APP_H

#include <stdint.h>

typedef struct CubacadabraApp CubacadabraApp;

// Handles are single-threaded. Copy output before the next mutating call.
CubacadabraApp *cubacadabra_app_create(void);
void cubacadabra_app_destroy(CubacadabraApp *app);
// Returns 1 for a valid action, 0 for invalid UTF-8/JSON/action (no state change).
uint8_t cubacadabra_app_dispatch_json(CubacadabraApp *app, const uint8_t *source, uintptr_t length);
uint8_t cubacadabra_app_snapshot_json(CubacadabraApp *app);
// Returns 1 with effect JSON, 0 with empty output when the queue is empty.
uint8_t cubacadabra_app_poll_effect_json(CubacadabraApp *app);
const uint8_t *cubacadabra_app_output_ptr(const CubacadabraApp *app);
uintptr_t cubacadabra_app_output_len(const CubacadabraApp *app);

#endif
