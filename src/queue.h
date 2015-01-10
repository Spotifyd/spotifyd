/*
 * spotifyd - A daemon playing music from spotify, in the spirit of MPD.
 * Copyright (C) 2015 Simon Persson
 * 
 * Spotifyd program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Spotifyd program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */
#pragma once

#include <libspotify/api.h>

pthread_mutex_t queue_lock;

int queue_get_next();
unsigned queue_get_len();
sp_track *queue_get(unsigned);
void queue_set_current(unsigned);
sp_track *queue_get_current();
bool queue_del_track(unsigned trackn);
bool queue_toggle_random();
bool queue_add_track(sp_track *track);
void queue_init();
