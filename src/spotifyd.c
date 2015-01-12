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
#include <string.h>
#include <libspotify/api.h>
#include <stdlib.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <pthread.h>
#include <errno.h>

#include "queue.h"
#include "helpers.h"
#include "socket.h"
#include "audio.h"
#include "spotifyd.h"
#include "session.h"
#include "commandq.h"
#include "commandq.h"
#include "search.h"

pthread_t accept_thread;

int main()
{
	read_config();

	sp_session *session = NULL;
	sp_error error;

	is_playing = 0;

	if(commandq_init() != 0)
	{
		printf("Couldn't create commandq.");
		exit(1);
	}

	audio_init(&g_audiofifo);

	pthread_mutex_init(&search_result_lock, NULL);

	queue_init();

	atexit(&cleanup);

	if((error = session_init(&session)) != SP_ERROR_OK)
	{
		printf("%s", sp_error_message(error));
	}

	if(have_socket_path())
	{	
		pthread_create(&accept_thread, NULL, sock_accept_connections_un, NULL);
	}
	if(have_port())
	{
		pthread_create(&accept_thread, NULL, sock_accept_connections_ip, NULL);
	}

	/* Main loop. Process spotify events and incoming socket connections. */
	pthread_mutex_init(&notify_mutex, NULL);
	pthread_mutex_lock(&notify_mutex);
	notify_do = 1;
	int next_timeout = 0;
	for(;;)
	{
		struct timespec ts = rel_to_abstime(next_timeout);
		
		while(notify_do == 0)
		{
			int error = pthread_cond_timedwait(&notify_cond, &notify_mutex, &ts);
			if(error == ETIMEDOUT)
			{
				/* 
				 * This means next_timeout was reached.
				 * Time to get out of here and do stuff.
				 */
				break;
			}
		}
		notify_do = 0;	
		pthread_mutex_unlock(&notify_mutex);
	
		/* 
		 * Executes the command on the top of the command queue,
		 * if there is one.
		 */
		pthread_mutex_lock(&commandq_lock);
		commandq_execute_front(session);
		pthread_mutex_unlock(&commandq_lock);
		
		do
		{
			sp_session_process_events(session, &next_timeout);
		} while(next_timeout == 0);

		pthread_mutex_lock(&notify_mutex);
	}
	return 0;
}

void cleanup()
{
	sock_close();
	while(commandq.tqh_first != NULL)
	{
		commandq_pop();
	}
	while(queue_del_track(0));
	search_clear();
}


