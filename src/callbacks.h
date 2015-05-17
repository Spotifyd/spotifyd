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

int on_music_delivered(sp_session *session, const sp_audioformat *format, const void *frames, int num_frames);
void on_notify_main_thread(sp_session *sess);
void on_albumbrowse_complete(sp_albumbrowse *result, void *userdata);
void on_end_of_track(sp_session *session);
void on_search_complete(sp_search *search, void *userdata);
void container_loaded(sp_playlistcontainer *pc, void *userdata);
void on_login(sp_session *session, sp_error error);
