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

void playlist_init(sp_session *session);
void playlist_container_loaded(sp_playlistcontainer *pc, void *userdata);
unsigned playlist_len();
void playlist_get_name(char *buf, size_t len, unsigned i);
bool playlist_for_each(unsigned playlistn, bool (*func_ptr)(sp_track *));
bool playlist_add_track(unsigned playlist, sp_track * const track, sp_session *session);
bool playlist_del_track(unsigned playlist, int track);
bool playlist_new(const char * const name);
bool playlist_remove(unsigned playlist);
