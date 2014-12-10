#pragma once
#include <libspotify/api.h>

int on_music_delivered(sp_session *session, const sp_audioformat *format, const void *frames, int num_frames);
void on_notify_main_thread(sp_session *sess);
void on_end_of_track(sp_session *session);
void on_search_complete(sp_search *search, void *userdata);
void container_loaded(sp_playlistcontainer *pc, void *userdata);
void on_login(sp_session *session, sp_error error);
