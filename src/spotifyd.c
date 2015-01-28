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
#include <signal.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/stat.h>

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
	/*
	 * Fork off and daemonize the program.
	 */
	daemonize();

	/*
	 * read username/password and where to listen for socket connections
	 * from config file or stdin.
	 */
	read_config();

	sp_session *session = NULL;
	sp_error error;
	is_playing = 0;

	/*
	 * Don't let the process die if the client hangs up on us.
	 */
	signal(SIGPIPE, SIG_IGN);

	/*
	 * set up the queue where commands from the user
	 * will be stored.
	 */
	if(commandq_init() != 0)
	{
		LOG_PRINT("Couldn't create commandq.");
		exit(1);
	}

	audio_init(&g_audiofifo);

	/*
	 * init the queue of songs to play.
	 */
	queue_init();

	/*
	 * make sure we free memory and close sockets 
	 * when the application closes.
	 */
	atexit(&cleanup);

	/*
	 * sign in to spotify.
	 */
	if((error = session_init(&session)) != SP_ERROR_OK)
	{
		LOG_PRINT("%s", sp_error_message(error));
	}

	/*
	 * if we have a path, listen to it as a unix socket.
	 * if we have a port, listen to connections over network.
	 */
	if(have_socket_path())
	{	
		pthread_create(&accept_thread, NULL, sock_accept_connections_un, NULL);
	}
	if(have_port())
	{
		pthread_create(&accept_thread, NULL, sock_accept_connections_ip, NULL);
	}

	/* Main loop. Process spotify events and incoming socket connections. */
	int mutex_init_error;
	do
	{
		mutex_init_error = pthread_mutex_init(&notify_mutex, NULL);
	} while(mutex_init_error == EAGAIN);
	if(mutex_init_error != 0)
	{
		LOG_PRINT("Couldn't initialize mutex. Quitting.\n");
	}

	do
	{
		mutex_init_error = pthread_cond_init(&notify_cond, NULL);
	} while(mutex_init_error == EAGAIN);
	if(mutex_init_error != 0)
	{
		LOG_PRINT("Couldn't initialize mutex. Quitting.\n");
	}

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

void daemonize()
{
	pid_t pid, sid;
	pid = fork();
	if(pid < 0)
	{
		exit(1);
	}
	
	if(pid > 0)
	{
		exit(0);
	}

	umask(0);
	
	sid = setsid();
	if(sid < 0)
	{
		exit(1);
	}

	if(chdir("/") < 0)
	{
		exit(1);
	}

	close(STDIN_FILENO);
	close(STDERR_FILENO);
	close(STDOUT_FILENO);
}

void cleanup()
{
	sock_close();
	while(commandq.tqh_first != NULL)
	{
		commandq_pop();
	}
	while(queue_del_track(0));
	if(get_logfile() != NULL)
	{
		fclose(get_logfile());
	}
	search_clear();
}


