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
#include <stdio.h>
#include <libspotify/api.h>

#include "callbacks.h"
#include "session.h"
#include "config.h"
#include "helpers.h"

static sp_session_callbacks session_callbacks = {
	.logged_in = &on_login,
	.music_delivery = &on_music_delivered,
	.notify_main_thread = &on_notify_main_thread,
	.end_of_track = &on_end_of_track,
};

static sp_session_config spconfig = {
	.api_version = SPOTIFY_API_VERSION,
	.cache_location = "/tmp/",
	.settings_location = "/tmp/",
	.application_key = NULL,
	.application_key_size = 0,
	.user_agent = "spotifyd",
	.callbacks = &session_callbacks,
};


sp_error session_init(sp_session **session)
{
	sp_error error;

	spconfig.application_key = &_binary_src_appkey_key_start;
	spconfig.application_key_size = (size_t)(&_binary_src_appkey_key_end - &_binary_src_appkey_key_start);

	if((error = sp_session_create(&spconfig, session)) != SP_ERROR_OK)
	{
		return error;
	}

	return sp_session_login(*session, USERNAME, PASSWORD, 0, NULL);
}

