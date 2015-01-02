#include <stdio.h>
#include <libspotify/api.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>

#include "audio.h"
#include "queue.h"
#include "socket.h"
#include "config.h"
#include "helpers.h"
#include "spotifyd.h"
#include "audio.h"

sp_playlistcontainer *playlist_container = NULL;
static sp_playlistcontainer_callbacks pc_callbacks = {
	.container_loaded = &container_loaded,
};

/*
 * from jukebox.c in the libspotify examples. Thanks spotify <3
 *
 * see audio.c for full license of this method.
 */
int on_music_delivered(sp_session *session, const sp_audioformat *format, const void *frames, int num_frames)
{
	debug("on_music_delivered\n");

	audio_fifo_t *af = &g_audiofifo;
	audio_fifo_data_t *afd;
	size_t s;

	if (num_frames == 0)
		return 0; // Audio discontinuity, do nothing
	
	pthread_mutex_lock(&af->mutex);

	/* Buffer one second of audio */
	if (af->qlen > format->sample_rate) {
		pthread_mutex_unlock(&af->mutex);
		return 0;
	}

	s = num_frames * sizeof(int16_t) * format->channels;

	afd = malloc(sizeof(*afd) + s);
	memcpy(afd->samples, frames, s);

	afd->nsamples = num_frames;

	afd->rate = format->sample_rate;
	afd->channels = format->channels;

	TAILQ_INSERT_TAIL(&af->q, afd, link);
	af->qlen += num_frames;

	pthread_cond_signal(&af->cond);
	pthread_mutex_unlock(&af->mutex);

	return num_frames;
}

void on_notify_main_thread(sp_session *sess)
{
	debug("on_notify_main_thread\n");
	notify_main_thread();
}

void on_end_of_track(sp_session *session)
{
	debug("on_end_of_track\n");
	sp_session_player_unload(session);
	/*
	 * Add a play command containing next song to be played
	 * to the command queue.
	 */
	struct commandq_entry *entry = malloc(sizeof(struct commandq_entry));
	struct command *command = malloc(sizeof(struct command));
	entry->val = command;
	command->type = QPLAY;	
	command->track = queue_get_next();
	commandq_insert(entry);
	pthread_mutex_lock(&commandq_lock);
	notify_main_thread();
	pthread_mutex_unlock(&commandq_lock);
}

void on_search_complete(sp_search *search, void *userdata)
{
	debug("on_search_complete\n");
	
	int i;

	/*
	 * Begin by releasing the previous search results.
	 */
	pthread_mutex_lock(&search_result_lock);
	for(i=0; i<NUM_SEARCH_RESULTS; ++i)
	{
		if(search_result[i] != NULL)
		{
			sp_track_release(search_result[i]);
			search_result[i] = NULL;
		}
	}

	sp_error error = sp_search_error(search);
	if (error != SP_ERROR_OK)
	{
		printf("Error: %s\n", sp_error_message(error));
		exit(1);
	}

	int num_tracks = sp_search_num_tracks(search);
	
	sp_track *track;

	pthread_mutex_lock(&commandq_lock);
	int sockfd = commandq.tqh_first->val->sockfd;

	/*
	 * Put store all the search results. Add one reference to them,
	 * as they loose one reference when the search is freed.
	 */
	for(i=0; i<num_tracks; ++i)
	{
		track = sp_search_track(search, i);
		sp_track_add_ref(track);
		search_result[i] = track;
		sock_send_track_with_trackn(sockfd, search_result[i], i);
	}
	
/*
	 * If we ended up here, that means that the first element on the
	 * commandq is a search. Set it to done and notify the main thread 
	 * so the search command can be freed.
	 */
	close(sockfd);
	commandq.tqh_first->val->done = 1;
	pthread_mutex_unlock(&commandq_lock);
	pthread_mutex_unlock(&search_result_lock);
	notify_main_thread();

	sp_search_release(search);
}

void container_loaded(sp_playlistcontainer *pc, void *userdata)
{
	debug("container_loaded\n");
	playlist_container=pc;
}

void on_login(sp_session *session, sp_error error)
{
	debug("on_login\n");
	if(error != SP_ERROR_OK)
	{
		printf("Couldn't log in.\n");
		exit (1);
	}
	sp_playlistcontainer *pc = sp_session_playlistcontainer(session);
	sp_playlistcontainer_add_callbacks(
		pc,
		&pc_callbacks,
		NULL
	);
}
