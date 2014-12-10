#include <libspotify/api.h>
#include <string.h>
#include <time.h>
#include <stdlib.h>

#include "config.h"
#include "queue.h"
#include "spotifyd.h"

void queue_init()
{
	pthread_mutex_init(&queue_lock, NULL);
	pthread_mutex_lock(&queue_lock);
	queue_len = 0;
	queue_position = 0;
	queue_random = 0;
	memset(queue, 0, PLAY_QUEUE_LEN * sizeof(sp_track *));
	pthread_mutex_unlock(&queue_lock);
}

void queue_add_track(sp_track *track)
{
	pthread_mutex_lock(&queue_lock);
	if(track != NULL)
	{
		sp_track_add_ref(track);
		queue[queue_len++] = track;
	}
	pthread_mutex_unlock(&queue_lock);
}

int queue_get_next()
{
	srand(time(NULL));
	pthread_mutex_lock(&queue_lock);
	int next_track;
	if(queue_random)
	{
		next_track = rand()%queue_len;
	}
	else
	{
		next_track = (queue_position + 1)%queue_len;
	}
	pthread_mutex_unlock(&queue_lock);
	return next_track;
}

bool queue_toggle_random()
{
	pthread_mutex_lock(&queue_lock);
	queue_random = !queue_random;
	pthread_mutex_unlock(&queue_lock);
	return queue_random;
}

void queue_del_track(unsigned trackn)
{
	pthread_mutex_lock(&queue_lock);
	if(trackn < queue_len && queue[trackn] != NULL)
	{
		sp_track_release(queue[trackn]);
		queue[trackn] = NULL;
		--queue_len;
		memmove(&queue[trackn], &queue[trackn+1], PLAY_QUEUE_LEN - trackn);
		memset(&queue[queue_len], 0, sizeof(sp_track *)*(PLAY_QUEUE_LEN - queue_len));
	}
	pthread_mutex_unlock(&queue_lock);
}
