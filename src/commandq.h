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
		QDEL, /* delete track in queue */
		PAUSE, /* toggle play/pause */	
		SEARCH,
		HELP /* send help text back on socket */
	} type;
	bool handled;
	bool done;
	int sockfd;
	union
	{
		char *search_string;
		long track;
	};
};

static const char help_str[] = "Usage:\n \
\t SEARCH str - Searches spotify for str.\n \
\t QLIST      - List content of the queue.\n \
\t SLIST      - List search results.\n \
\t QRAND      - Toggle queue randomness on/off.\n \
\t QADD n     - Add song n from search results to queue.\n \
\t QCLEAR     - Clear the queue.\n \
\t PLAY n     - Play song n in queue.\n";

void commandq_pop();
int commandq_init();
void commandq_execute_front();
void commandq_insert(struct commandq_entry *entry);
void commandq_free_entry(struct commandq_entry *e);
void commandq_execute_command(sp_session *session, struct command *command);
