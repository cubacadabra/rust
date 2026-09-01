#ifndef CUBACADABRA_ENGINE_H
#define CUBACADABRA_ENGINE_H

#include <stdint.h>

typedef struct CubacadabraEngine CubacadabraEngine;

CubacadabraEngine *engine_create(void);
void engine_set_input(
    CubacadabraEngine *engine,
    float forward,
    float strafe,
    uint8_t sprint,
    uint8_t jump,
    float look_x,
    float look_y,
    float zoom_delta
);
void engine_step(CubacadabraEngine *engine, float delta);
void engine_reset_view(CubacadabraEngine *engine);
const float *engine_snapshot_ptr(const CubacadabraEngine *engine);
uintptr_t engine_snapshot_len(void);
uintptr_t engine_snapshot_stride(void);
float engine_camera_yaw(const CubacadabraEngine *engine);
float engine_camera_pitch(const CubacadabraEngine *engine);
float engine_camera_distance(const CubacadabraEngine *engine);
uintptr_t engine_agent_count(const CubacadabraEngine *engine);
uintptr_t engine_meeting_count(const CubacadabraEngine *engine, uintptr_t index);
float engine_elapsed(const CubacadabraEngine *engine);
void engine_destroy(CubacadabraEngine *engine);

#endif
