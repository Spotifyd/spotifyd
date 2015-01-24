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
#include <libspotify/api.h>
#include <string.h>
#include <time.h>
#include <stdlib.h>
#include <stdio.h>

#include "config.h"
#include "queue.h"
#include "spotifyd.h"

sp_track *queue[PLAY_QUEUE_LEN];
sp_track *cur_playing;

unsigned queue_len;
unsigned queue_position;

void queue_init()
{
	srand(time(NULL));
	cur_playing = NULL;
	/*
	 * Print first song in queue first.
	 */
	queue_print_cur_first = 0;
	queue_len = 0;
	queue_position = 0;
	memset(queue, 0, PLAY_QUEUE_LEN * sizeof(sp_track *));
}

void queue_shuffle()
{
	int i;
	/*
	 * run five times the queue length to make sure most tracks
	 * are moved.
	 */
	for(i=0; i<5*queue_len; ++i)
	{
		/*
		 * take two random tracks and swap them.
		 */
		int p1 = rand()%queue_len, p2 = rand()%queue_len;
		sp_track *tmp = queue[p1];
		queue[p1] = queue[p2];
		queue[p2] = tmp;
	}
}

bool queue_add_track(sp_track *track)
{
	bool track_added;

	if(track != NULL && queue_len + 1< PLAY_QUEUE_LEN)
	{
		sp_track_add_ref(track);
		queue[queue_len++] = track;
		track_added = 1;
	}
	else
	{
		track_added = 0;
	}

	return track_added;
}

int queue_get_next()
{
	int next_track;
	/* Avoid division by zero */
	if(queue_len > 0)
	{
		next_track = (queue_position + 1)%queue_len;
	}
	return next_track;
}

int queue_get_prev()
{
	int prev_track;
	if(queue_len > 0)
	{
		prev_track = (queue_position - 1)%queue_len;
	}
	return prev_track;
}

sp_track *queue_get(unsigned i)
{
	if(i < queue_len)
		return queue[i];
	else
		return NULL;
}

void queue_set_current(unsigned i)
{
	if(cur_playing != NULL)
	{
		sp_track_release(cur_playing);
	}
	queue_position = i;
	sp_track_add_ref(queue[i]);
	cur_playing = queue[i];
}

sp_track *queue_get_current()
{
	return cur_playing;
}

int queue_get_pos()
{
	return queue_position;
}

unsigned queue_get_len()
{
	return queue_len;
}

bool queue_del_track(unsigned trackn)
{
	bool ret_val = 0;

	if(trackn < queue_len && queue[trackn] != NULL)
	{
		sp_track_release(queue[trackn]);
		queue[trackn] = NULL;
		--queue_len;
		memmove(&queue[trackn], &queue[trackn+1], PLAY_QUEUE_LEN - trackn);
		memset(&queue[queue_len], 0, sizeof(sp_track *)*(PLAY_QUEUE_LEN - queue_len));
		ret_val = 1;
	}
	return ret_val;
}
