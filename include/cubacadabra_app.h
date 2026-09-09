#ifndef CUBACADABRA_APP_H
#define CUBACADABRA_APP_H

#include <stdint.h>

typedef struct CubacadabraApp CubacadabraApp;

#define CUBACADABRA_APP_EFFECT_NONE 0
#define CUBACADABRA_APP_EFFECT_SAVE_USERNAME 1

CubacadabraApp *cubacadabra_app_create(
    const uint8_t *username,
    uintptr_t username_length
);
void cubacadabra_app_destroy(CubacadabraApp *app);
uint8_t cubacadabra_app_replace_profile(
    CubacadabraApp *app,
    const uint8_t *username,
    uintptr_t username_length
);
uint8_t cubacadabra_app_username_changed(
    CubacadabraApp *app,
    const uint8_t *value,
    uintptr_t value_length
);
void cubacadabra_app_save_username(CubacadabraApp *app);
uint8_t cubacadabra_app_username_saved(
    CubacadabraApp *app,
    uint32_t effect_id,
    const uint8_t *username,
    uintptr_t username_length
);
uint8_t cubacadabra_app_username_save_failed(
    CubacadabraApp *app,
    uint32_t effect_id,
    const uint8_t *server_code,
    uintptr_t server_code_length
);
void cubacadabra_app_clear_username_feedback(CubacadabraApp *app);
uint8_t cubacadabra_app_snapshot_json(CubacadabraApp *app);
uint8_t cubacadabra_app_poll_effect(CubacadabraApp *app);
uint32_t cubacadabra_app_effect_id(const CubacadabraApp *app);
const uint8_t *cubacadabra_app_output_ptr(const CubacadabraApp *app);
uintptr_t cubacadabra_app_output_len(const CubacadabraApp *app);

#endif
