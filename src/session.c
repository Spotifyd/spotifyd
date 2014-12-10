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

