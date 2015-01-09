#pragma once

#include <libspotify/api.h>

pthread_mutex_t queue_lock;

int queue_get_next();
unsigned queue_get_len();
sp_track *queue_get(unsigned);
void queue_set_current(unsigned);
sp_track *queue_get_current();
void queue_del_track(unsigned trackn);
bool queue_toggle_random();
bool queue_add_track(sp_track *track);
void queue_init();
