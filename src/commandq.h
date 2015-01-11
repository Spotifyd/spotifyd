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

#include <pthread.h>
#include <sys/queue.h>
#include <libspotify/api.h>

pthread_mutex_t commandq_lock;
TAILQ_HEAD(tailhead, commandq_entry) commandq;
struct commandq_entry
{
	TAILQ_ENTRY(commandq_entry) entries;
	struct command *val;
};

struct command
{
	enum {
		SLIST, /* show search results */
		QLIST, /* show queue */
		QRAND, /* toggle queue randomness */
		PLAY, /* play song from queue */
		QCLEAR, /* clear queue */
		QADD, /* add search result to queue */
		QRM, /* remove track in queue */
		PAUSE, /* toggle play/pause */	
		SEARCH, /* search for songs on spotify */
		CUR_PLAYING, /* return currently playing song */
		HELP, /* send help text back on socket */
		PL, /* list available playlists */
		SADDPL, /* put playlist to search list */
		QADDPL /* put playlist in queue */
	} type;
	bool handled;
	bool done;
	int sockfd;
	union
	{
		char *search_string;
		long track;
		int playlist;
	};
};

static const char help_str[] = "Usage:\n \
\t SEARCH str  - Searches spotify for str.\n \
\t CUR_PLAYING - Returns the currently playing song.\n \
\t QLIST       - List content of the queue.\n \
\t SLIST       - List search results.\n \
\t QRAND       - Toggle queue randomness on/off.\n \
\t QADD n      - Add song n from search results to queue.\n \
\t QCLEAR      - Clear the queue.\n \
\t QRM n       - Remove track n from queue.\n \
\t PLAY n      - Play song n in queue.\n \
\t PAUSE       - Toggle play/pause.\n \
\t PL          - List available playlists.\n \
\t SADDPL n    - Put playlist n in search list.\n \
\t QADDPL n    - Put playlist n in queue.\n";

void commandq_pop();
int commandq_init();
void commandq_execute_front();
void commandq_insert(struct commandq_entry *entry);
void commandq_free_entry(struct commandq_entry *e);
void commandq_execute_command(sp_session *session, struct command *command);
