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

#define FOREACH_COMMAND(COMMAND) \
		COMMAND(SLIST) \
		COMMAND(QLIST) \
		COMMAND(QRAND) \
		COMMAND(QPRINT) \
		COMMAND(PLAY) \
		COMMAND(PREV) \
		COMMAND(NEXT) \
		COMMAND(QCLEAR) \
		COMMAND(QADD) \
		COMMAND(QRM) \
		COMMAND(PAUSE) \
		COMMAND(SEARCH) \
		COMMAND(CUR_PLAYING) \
		COMMAND(HELP) \
		COMMAND(PL) \
		COMMAND(QADDPL) \
		COMMAND(PLADD) \
		COMMAND(PLCREATE) \
		COMMAND(PLDELETE) \
		COMMAND(PLRM) \
		COMMAND(LINK) \
		COMMAND(VOL) \

#define GENERATE_ENUM(ENUM) ENUM,
#define GENERATE_STRING(STRING) #STRING,

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
		FOREACH_COMMAND(GENERATE_ENUM)
	} type;
	bool handled;
	bool done;
	int sockfd;
	union
	{
		char *search_string;
		char *name;
		unsigned track;
		unsigned volume;
	};
	int playlist;
};

__attribute__((unused))
static const char *COMMAND_STR[] = { FOREACH_COMMAND(GENERATE_STRING)  };

static const char help_str[] = "Usage:\n \
\t SEARCH str  - Searches spotify for str.\n \
\t CUR_PLAYING - Returns the currently playing song.\n \
\t QLIST       - List content of the queue.\n \
\t SLIST       - List search results.\n \
\t QRAND       - Shuffle queue.\n \
\t QADD n      - Add song n from search results to queue.\n \
\t QCLEAR      - Clear the queue.\n \
\t QPRINT      - Toggle queue printing behaviour.\n \
\t QRM n       - Remove track n from queue.\n \
\t PLAY n      - Play song n in queue.\n \
\t PREV        - Play previous song.\n \
\t NEXT        - Play next song.\n \
\t PAUSE       - Toggle play/pause.\n \
\t PL          - List available playlists.\n \
\t PLCREATE s  - Create new playlist with name s.\n \
\t PLDELETE n  - Delete playlist n.\n \
\t QADDPL n    - Put playlist n in queue.\n \
\t PLADD n p   - Add track n from queue to playlist p. \n \
\t LINK s      - Looks up spotify uri s and adds to search list. \n \
\t vol p       - Sets volume to percentage p (0 to 100). \n \
\t PLRM n p    - Remove track n from playlist p.\n";

void commandq_pop();
int commandq_init();
void commandq_execute_front();
void commandq_insert(struct commandq_entry *entry);
void commandq_free_entry(struct commandq_entry *e);
void commandq_execute_command(sp_session *session, struct command *command);
