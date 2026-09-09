#ifndef CUBACADABRA_CLIENT_H
#define CUBACADABRA_CLIENT_H

#include <stdint.h>

#include "cubacadabra_engine.h"

typedef struct CubacadabraClient CubacadabraClient;

#define CUBACADABRA_CLIENT_ACTION_NONE 0
#define CUBACADABRA_CLIENT_ACTION_SET_WORLD 1
#define CUBACADABRA_CLIENT_ACTION_SEND_TEXT 2

CubacadabraClient *client_create(
    const uint8_t *manifest,
    uintptr_t manifest_length,
    const uint8_t *script,
    uintptr_t script_length
);
void client_destroy(CubacadabraClient *client);
CubacadabraEngine *client_engine(CubacadabraClient *client);
void client_transport_connected(CubacadabraClient *client);
void client_transport_disconnected(CubacadabraClient *client);
void client_request_transport(CubacadabraClient *client);
uint8_t client_receive_text(
    CubacadabraClient *client,
    const uint8_t *message,
    uintptr_t message_length
);
uint8_t client_set_ignored_player_ids_json(
    CubacadabraClient *client,
    const uint8_t *player_ids,
    uintptr_t player_ids_length
);
uint8_t client_poll_action(CubacadabraClient *client);
const uint8_t *client_action_ptr(const CubacadabraClient *client);
uintptr_t client_action_len(const CubacadabraClient *client);

#endif
