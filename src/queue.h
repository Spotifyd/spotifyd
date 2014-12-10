#pragma once

#include <libspotify/api.h>

int queue_get_next();
void queue_del_track(unsigned trackn);
bool queue_toggle_random();
void queue_add_track(sp_track *track);
void queue_init();
