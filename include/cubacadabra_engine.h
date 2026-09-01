#ifndef CUBACADABRA_ENGINE_H
#define CUBACADABRA_ENGINE_H

#include <stdint.h>

typedef struct CubacadabraEngine CubacadabraEngine;
typedef struct CubacadabraRenderer CubacadabraRenderer;

typedef struct {
    float position[3];
    float size[3];
    float color[4];
} CubacadabraRenderBlock;

typedef struct {
    float x;
    float z;
    float radius;
    float seconds;
    float color[4];
} CubacadabraRenderPad;

typedef struct {
    float position[3];
    float yaw;
    float walk_cycle;
    float assembled;
    float skin[4];
    float shirt[4];
    float pants[4];
    float shoes[4];
} CubacadabraRenderAgent;

typedef struct {
    float sky[4];
    float ground[4];
    float ground_edge[4];
    float grid[4];
    float ink[4];
} CubacadabraRenderPalette;

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
void engine_set_launch_pad(
    CubacadabraEngine *engine,
    uintptr_t index,
    float x,
    float z,
    float radius,
    float countdown
);
void engine_set_launch_pad_count(CubacadabraEngine *engine, uintptr_t count);
void engine_set_obstacle(
    CubacadabraEngine *engine,
    uintptr_t index,
    float x,
    float y,
    float z,
    float width,
    float height,
    float depth
);
void engine_set_obstacle_count(CubacadabraEngine *engine, uintptr_t count);
void engine_set_world_count(CubacadabraEngine *engine, uintptr_t count);
void engine_set_world_spawn(
    CubacadabraEngine *engine,
    uintptr_t world,
    float x,
    float y,
    float z
);
void engine_set_world_launch_pad_count(
    CubacadabraEngine *engine,
    uintptr_t world,
    uintptr_t count
);
void engine_set_world_launch_pad(
    CubacadabraEngine *engine,
    uintptr_t world,
    uintptr_t index,
    float x,
    float z,
    float radius,
    float countdown
);
void engine_set_world_launch_destination(
    CubacadabraEngine *engine,
    uintptr_t world,
    uintptr_t pad,
    int32_t destination
);
void engine_set_world_obstacle_count(
    CubacadabraEngine *engine,
    uintptr_t world,
    uintptr_t count
);
void engine_set_world_obstacle(
    CubacadabraEngine *engine,
    uintptr_t world,
    uintptr_t index,
    float x,
    float y,
    float z,
    float width,
    float height,
    float depth
);
uint8_t engine_start_world(CubacadabraEngine *engine, uintptr_t world);
uintptr_t engine_enter_session(
    CubacadabraEngine *engine,
    uintptr_t launch_pad_index,
    float spawn_x,
    float spawn_y,
    float spawn_z
);
uint8_t *engine_script_buffer_ptr(CubacadabraEngine *engine, uintptr_t length);
uint8_t engine_load_script_buffer(CubacadabraEngine *engine);
uint8_t engine_script_loaded(const CubacadabraEngine *engine);
const float *engine_snapshot_ptr(const CubacadabraEngine *engine);
uintptr_t engine_snapshot_len(void);
uintptr_t engine_snapshot_stride(void);
float engine_camera_yaw(const CubacadabraEngine *engine);
float engine_camera_pitch(const CubacadabraEngine *engine);
float engine_camera_distance(const CubacadabraEngine *engine);
uintptr_t engine_agent_count(const CubacadabraEngine *engine);
uintptr_t engine_meeting_count(const CubacadabraEngine *engine, uintptr_t index);
uintptr_t engine_launch_pad_count(const CubacadabraEngine *engine);
uintptr_t engine_launch_pad_occupants(const CubacadabraEngine *engine, uintptr_t index);
float engine_launch_pad_seconds(const CubacadabraEngine *engine, uintptr_t index);
uint8_t engine_launch_pad_phase(const CubacadabraEngine *engine, uintptr_t index);
int32_t engine_player_launch_pad(const CubacadabraEngine *engine);
uint32_t engine_launch_event_id(const CubacadabraEngine *engine);
uintptr_t engine_last_launch_pad(const CubacadabraEngine *engine);
uintptr_t engine_last_launch_occupants(const CubacadabraEngine *engine);
uintptr_t engine_active_world(const CubacadabraEngine *engine);
uint32_t engine_world_event_id(const CubacadabraEngine *engine);
uintptr_t engine_last_world_source_pad(const CubacadabraEngine *engine);
uintptr_t engine_last_world_destination(const CubacadabraEngine *engine);
float engine_elapsed(const CubacadabraEngine *engine);
void engine_destroy(CubacadabraEngine *engine);

CubacadabraRenderer *engine_renderer_create(void *metal_layer, float width, float height);
void engine_renderer_resize(CubacadabraRenderer *renderer, float width, float height);
void engine_renderer_set_scene(
    CubacadabraRenderer *renderer,
    const CubacadabraRenderBlock *blocks,
    uintptr_t block_count,
    const CubacadabraRenderPad *pads,
    uintptr_t pad_count,
    const CubacadabraRenderAgent *agents,
    uintptr_t agent_count,
    CubacadabraRenderAgent player,
    float ground_size,
    CubacadabraRenderPalette palette,
    const float *camera,
    float elapsed
);
void engine_renderer_draw(CubacadabraRenderer *renderer);
void engine_renderer_destroy(CubacadabraRenderer *renderer);

#endif
