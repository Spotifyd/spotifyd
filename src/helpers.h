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
#include "spotifyd.h"
#include "time.h"

#define DEBUG 0

#define LOG_PRINT(...) if(get_logfile() != NULL) { fprintf(get_logfile(), __VA_ARGS__); fflush(get_logfile()); }

void track_to_str(char *buf, size_t len, sp_track *);
void album_to_str(char *buf, size_t len, sp_album *);
void playlist_to_str(char *buf, size_t len, sp_playlist *);
void notify_main_thread();
struct timespec rel_to_abstime(int msec);
void debug(const char *debug_msg);
bool play(sp_session *session, sp_track *track, bool flush);
